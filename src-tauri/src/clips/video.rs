//! Frames to H.264 on the GPU, through Windows' own hardware encoder (a Media Foundation MFT):
//! each BGRA frame is scaled and letterboxed into a fixed 1920×1080 NV12 texture by the D3D11
//! video processor, then fed to the async encoder. Compressed samples come back for the ring.
//! Call everything on one MTA thread (`Encoder` is not shared).

use std::mem::ManuallyDrop;

use windows::Win32::Foundation::{HMODULE, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION, D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_COLOR_SPACE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0,
    D3D11_VIDEO_PROCESSOR_STREAM, D3D11_VIDEO_USAGE_PLAYBACK_NORMAL, D3D11_VPIV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2D, D3D11CreateDevice,
    ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D, ID3D11VideoContext, ID3D11VideoDevice, ID3D11VideoProcessor,
    ID3D11VideoProcessorEnumerator, ID3D11VideoProcessorInputView, ID3D11VideoProcessorOutputView,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_RATIONAL, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Media::MediaFoundation::{
    CODECAPI_AVEncCommonMeanBitRate, CODECAPI_AVEncCommonRateControlMode, CODECAPI_AVEncMPVDefaultBPictureCount, CODECAPI_AVEncMPVGOPSize,
    CODECAPI_AVEncVideoForceKeyFrame, CODECAPI_AVLowLatencyMode, ICodecAPI, IMFActivate, IMFAttributes, IMFDXGIDeviceManager, IMFMediaEventGenerator,
    IMFSample, IMFShutdown, IMFTransform, METransformHaveOutput, METransformNeedInput, MF_E_NO_EVENTS_AVAILABLE, MF_E_TRANSFORM_STREAM_CHANGE,
    MF_EVENT_FLAG_NO_WAIT, MF_LOW_LATENCY, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE,
    MF_MT_MPEG2_PROFILE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MF_TRANSFORM_ASYNC_UNLOCK, MFCreateAttributes, MFCreateDXGIDeviceManager,
    MFCreateDXGISurfaceBuffer, MFCreateMediaType, MFCreateSample, MFMediaType_Video, MFSampleExtension_CleanPoint, MFT_CATEGORY_VIDEO_ENCODER,
    MFT_ENUM_ADAPTER_LUID, MFT_ENUM_FLAG_HARDWARE, MFT_ENUM_FLAG_SORTANDFILTER, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_END_OF_STREAM,
    MFT_MESSAGE_NOTIFY_START_OF_STREAM, MFT_MESSAGE_SET_D3D_MANAGER, MFT_OUTPUT_DATA_BUFFER, MFT_REGISTER_TYPE_INFO, MFTEnum2, MFVideoFormat_H264,
    MFVideoFormat_NV12, MFVideoInterlace_Progressive, eAVEncCommonRateControlMode_CBR, eAVEncH264VProfile_Main,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Variant::VARIANT;
use windows::core::{Error, IUnknown, Interface, Result};

pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;
pub const FPS: u32 = 60;
const BITRATE: u32 = 12_000_000;
/// 100 ns units, as Media Foundation and the capture clock count.
pub const SECOND: i64 = 10_000_000;
const FRAME: i64 = SECOND / FPS as i64;
/// A keyframe at least this often, by time: the frame rate varies (capped clients, gaps).
const KEY_EVERY: i64 = 2 * SECOND;
/// NV12 textures the encoder may hold at once; with none free a frame is dropped.
const SLOTS: usize = 6;

/// One compressed sample: when it was shown, whether it starts a GOP, its Annex B bytes.
pub struct Encoded {
    pub time: i64,
    pub key: bool,
    pub data: Vec<u8>,
}

/// A Direct3D device the capture, the video processor and the encoder share.
pub struct Gpu {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
}

/// On the default graphics card, the one that shows the desktop (where the capture's frames are).
pub fn gpu() -> Result<Gpu> {
    let (mut device, mut context) = (None, None);
    // SAFETY: plain device creation; both out-pointers are locals.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
    }
    let device: ID3D11Device = device.ok_or_else(Error::empty)?;
    let context = context.ok_or_else(Error::empty)?;
    // The encoder works on the device from its own threads.
    // SAFETY: a flag on our own device.
    unsafe {
        let _ = device.cast::<ID3D11Multithread>()?.SetMultithreadProtected(true);
    }
    Ok(Gpu { device, context })
}

struct Slot {
    texture: ID3D11Texture2D,
    view: ID3D11VideoProcessorOutputView,
}

/// The video processor for one input size, with a texture frames are copied into first (a
/// capture's own texture can't always be the processor's input).
struct Proc {
    width: u32,
    height: u32,
    processor: ID3D11VideoProcessor,
    input: ID3D11Texture2D,
    view: ID3D11VideoProcessorInputView,
    slots: Vec<Slot>,
}

pub struct Encoder {
    gpu: Gpu,
    video_device: ID3D11VideoDevice,
    video_context: ID3D11VideoContext,
    mft: IMFTransform,
    codec: Option<ICodecAPI>,
    events: IMFMediaEventGenerator,
    _manager: IMFDXGIDeviceManager,
    proc: Option<Proc>,
    /// Inputs the encoder asked for and hasn't had.
    need: u32,
    last_key: Option<i64>,
    last_time: Option<i64>,
    /// The encoder's output type as a blob (`MFGetAttributesAsBlob`), with its SPS/PPS once known.
    out_type: Vec<u8>,
}

impl Encoder {
    /// The hardware H.264 encoder on `gpu`'s graphics card. Err: there is none.
    pub fn new(gpu: Gpu) -> Result<Self> {
        let mft = hardware_h264(&gpu.device).map_err(at("find encoder"))?;
        // SAFETY: setting up the MFT as Microsoft documents for async hardware encoders: unlock,
        // its Direct3D manager, the output type before the input type.
        unsafe {
            let attrs = mft.GetAttributes()?;
            attrs.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1).map_err(at("unlock"))?;
            let _ = attrs.SetUINT32(&MF_LOW_LATENCY, 1);
            let (mut token, mut manager) = (0u32, None);
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager: IMFDXGIDeviceManager = manager.ok_or_else(Error::empty)?;
            manager.ResetDevice(&gpu.device, token).map_err(at("reset device"))?;
            mft.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, manager.as_raw() as usize).map_err(at("set D3D manager"))?;

            let codec = mft.cast::<ICodecAPI>().ok();
            if let Some(c) = &codec {
                // Optional on some drivers: what they refuse they leave at their default.
                let _ = c.SetValue(&CODECAPI_AVEncCommonRateControlMode, &VARIANT::from(eAVEncCommonRateControlMode_CBR.0 as u32));
                let _ = c.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &VARIANT::from(BITRATE));
                let _ = c.SetValue(&CODECAPI_AVEncMPVGOPSize, &VARIANT::from(2 * FPS));
                let _ = c.SetValue(&CODECAPI_AVEncMPVDefaultBPictureCount, &VARIANT::from(0u32));
                let _ = c.SetValue(&CODECAPI_AVLowLatencyMode, &VARIANT::from(true));
            }

            let out = MFCreateMediaType()?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, BITRATE)?;
            out.SetUINT64(&MF_MT_FRAME_SIZE, pair(WIDTH, HEIGHT))?;
            out.SetUINT64(&MF_MT_FRAME_RATE, pair(FPS, 1))?;
            out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pair(1, 1))?;
            out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Main.0 as u32)?;
            mft.SetOutputType(0, &out, 0).map_err(at("output type"))?;

            let input = MFCreateMediaType()?;
            input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            input.SetUINT64(&MF_MT_FRAME_SIZE, pair(WIDTH, HEIGHT))?;
            input.SetUINT64(&MF_MT_FRAME_RATE, pair(FPS, 1))?;
            input.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pair(1, 1))?;
            input.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            mft.SetInputType(0, &input, 0).map_err(at("input type"))?;

            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0).map_err(at("begin streaming"))?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0).map_err(at("start of stream"))?;
            let events = mft.cast::<IMFMediaEventGenerator>()?;
            let out_type = super::mux::type_blob(&mft.GetOutputCurrentType(0)?)?;
            Ok(Encoder {
                video_device: gpu.device.cast()?,
                video_context: gpu.context.cast()?,
                gpu,
                mft,
                codec,
                events,
                _manager: manager,
                proc: None,
                need: 0,
                last_key: None,
                last_time: None,
                out_type,
            })
        }
    }

    pub fn device(&self) -> &ID3D11Device {
        &self.gpu.device
    }

    /// The encoder's output type (a blob), for the MP4 writer.
    pub fn out_type(&self) -> &[u8] {
        &self.out_type
    }

    /// Encodes `frame` (BGRA, any size) shown at `time`. False: dropped (the encoder isn't ready
    /// for one, or every texture is still in use).
    pub fn push(&mut self, frame: &ID3D11Texture2D, time: i64) -> Result<bool> {
        if self.need == 0 {
            return Ok(false);
        }
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: reads the frame's description.
        unsafe { frame.GetDesc(&mut desc) };
        if desc.Width == 0 || desc.Height == 0 {
            return Ok(false);
        }
        if !self.proc.as_ref().is_some_and(|p| p.width == desc.Width && p.height == desc.Height) {
            self.proc = Some(self.make_proc(desc.Width, desc.Height)?);
        }
        let Some(p) = &self.proc else { return Ok(false) };
        let Some(slot) = p.slots.iter().find(|s| only_ours(&s.texture)) else { return Ok(false) };
        let (dest, gap) = (fit(desc.Width, desc.Height), self.last_time.is_some_and(|t| time - t > SECOND));
        // SAFETY: copies between textures of one size on our device, then one processor pass
        // into an NV12 texture no one else holds (`only_ours`).
        unsafe {
            self.gpu.context.CopyResource(&p.input, frame);
            let full = RECT { left: 0, top: 0, right: desc.Width as i32, bottom: desc.Height as i32 };
            self.video_context.VideoProcessorSetStreamSourceRect(&p.processor, 0, true, Some(&full));
            self.video_context.VideoProcessorSetStreamDestRect(&p.processor, 0, true, Some(&dest));
            self.video_context.VideoProcessorSetStreamFrameFormat(&p.processor, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
            let stream = D3D11_VIDEO_PROCESSOR_STREAM { Enable: true.into(), pInputSurface: ManuallyDrop::new(Some(p.view.clone())), ..Default::default() };
            let streams = [stream];
            let blt = self.video_context.VideoProcessorBlt(&p.processor, &slot.view, 0, &streams);
            let [mut stream] = streams;
            ManuallyDrop::drop(&mut stream.pInputSurface);
            blt?;

            let buffer = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &slot.texture, 0, false)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(FRAME)?;
            if gap || self.last_key.is_none_or(|k| time - k >= KEY_EVERY) {
                if let Some(c) = &self.codec {
                    let _ = c.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &VARIANT::from(1u32));
                }
                self.last_key = Some(time);
            }
            self.mft.ProcessInput(0, &sample, 0)?;
        }
        self.need -= 1;
        self.last_time = Some(time);
        Ok(true)
    }

    /// Handles what the encoder said since the last call: inputs it wants, outputs it has.
    pub fn poll(&mut self, out: &mut Vec<Encoded>) -> Result<()> {
        loop {
            // SAFETY: asks the MFT's event queue without waiting.
            let event = match unsafe { self.events.GetEvent(MF_EVENT_FLAG_NO_WAIT) } {
                Ok(e) => e,
                Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => return Ok(()),
                Err(e) => return Err(e),
            };
            // SAFETY: reads the event's type and status.
            let (kind, status) = unsafe { (event.GetType()?, event.GetStatus()?) };
            status.ok()?;
            if kind == METransformNeedInput.0 as u32 {
                self.need += 1;
            } else if kind == METransformHaveOutput.0 as u32 {
                self.output(out)?;
            }
        }
    }

    fn output(&mut self, out: &mut Vec<Encoded>) -> Result<()> {
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER { dwStreamID: 0, pSample: ManuallyDrop::new(None), dwStatus: 0, pEvents: ManuallyDrop::new(None) }];
        let mut status = 0;
        // SAFETY: the encoder provides its own output samples (hardware MFTs do); what it hands
        // back in the buffer is taken out of the ManuallyDrop and dropped normally.
        let done = unsafe { self.mft.ProcessOutput(0, &mut buffers, &mut status) };
        let [mut b] = buffers;
        // SAFETY: each field is taken once.
        let (sample, events) = unsafe { (ManuallyDrop::take(&mut b.pSample), ManuallyDrop::take(&mut b.pEvents)) };
        drop(events);
        match done {
            Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                // SAFETY: takes the type the encoder now offers, as it asks.
                unsafe {
                    let t = self.mft.GetOutputAvailableType(0, 0)?;
                    self.mft.SetOutputType(0, &t, 0)?;
                    self.out_type = super::mux::type_blob(&self.mft.GetOutputCurrentType(0)?)?;
                }
                return Ok(());
            }
            Err(e) => return Err(e),
            Ok(()) => {}
        }
        let Some(sample) = sample else { return Ok(()) };
        out.push(read_sample(&sample)?);
        // The sequence header (SPS/PPS) lands in the output type with the first sample.
        // SAFETY: reads the current type.
        if let Ok(t) = unsafe { self.mft.GetOutputCurrentType(0) } {
            self.out_type = super::mux::type_blob(&t)?;
        }
        Ok(())
    }

    fn make_proc(&self, width: u32, height: u32) -> Result<Proc> {
        let content = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: DXGI_RATIONAL { Numerator: FPS, Denominator: 1 },
            InputWidth: width,
            InputHeight: height,
            OutputFrameRate: DXGI_RATIONAL { Numerator: FPS, Denominator: 1 },
            OutputWidth: WIDTH,
            OutputHeight: HEIGHT,
            Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
        };
        // SAFETY: creates processor objects and textures on our device from local descriptions.
        unsafe {
            let enumerator: ID3D11VideoProcessorEnumerator = self.video_device.CreateVideoProcessorEnumerator(&content)?;
            let processor = self.video_device.CreateVideoProcessor(&enumerator, 0)?;
            // Black bars, full-range RGB in; the processor picks BT.709 for the NV12 out.
            self.video_context.VideoProcessorSetOutputBackgroundColor(&processor, false, &Default::default());
            self.video_context.VideoProcessorSetStreamColorSpace(&processor, 0, &D3D11_VIDEO_PROCESSOR_COLOR_SPACE::default());
            self.video_context.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
            let input = texture(&self.gpu.device, width, height, DXGI_FORMAT_B8G8R8A8_UNORM)?;
            let in_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                FourCC: 0,
                ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPIV { MipSlice: 0, ArraySlice: 0 } },
            };
            let mut view = None;
            self.video_device.CreateVideoProcessorInputView(&input, &enumerator, &in_desc, Some(&mut view))?;
            let out_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 } },
            };
            let mut slots = Vec::with_capacity(SLOTS);
            for _ in 0..SLOTS {
                let texture = texture(&self.gpu.device, WIDTH, HEIGHT, DXGI_FORMAT_NV12)?;
                let mut view = None;
                self.video_device.CreateVideoProcessorOutputView(&texture, &enumerator, &out_desc, Some(&mut view))?;
                slots.push(Slot { texture, view: view.ok_or_else(Error::empty)? });
            }
            Ok(Proc { width, height, processor, input, view: view.ok_or_else(Error::empty)?, slots })
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: ends the stream and shuts the async MFT down, as it requires before release.
        unsafe {
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            if let Ok(s) = self.mft.cast::<IMFShutdown>() {
                let _ = s.Shutdown();
            }
        }
    }
}

/// An error that says which setup step failed.
fn at(step: &'static str) -> impl FnOnce(Error) -> Error {
    move |e| Error::new(e.code(), format!("{step}: {}", e.message()))
}

/// `hi` in the upper 32 bits, `lo` in the lower: how MF packs sizes and rates (pure).
fn pair(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

/// Where a `w`×`h` picture goes in the 1920×1080 frame: as large as fits, centred (pure).
fn fit(w: u32, h: u32) -> RECT {
    let (w, h) = (w.max(1) as u64, h.max(1) as u64);
    let (fw, fh) = if w * HEIGHT as u64 > h * WIDTH as u64 { (WIDTH as u64, (h * WIDTH as u64 / w) & !1) } else { ((w * HEIGHT as u64 / h) & !1, HEIGHT as u64) };
    let (x, y) = ((WIDTH as u64 - fw) / 2, (HEIGHT as u64 - fh) / 2);
    RECT { left: x as i32, top: y as i32, right: (x + fw) as i32, bottom: (y + fh) as i32 }
}

fn texture(device: &ID3D11Device, width: u32, height: u32, format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut t = None;
    // SAFETY: a texture from a local description.
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut t))? };
    t.ok_or_else(Error::empty)
}

/// Only the pool holds `t`: the encoder has let go of every sample made from it.
fn only_ours(t: &ID3D11Texture2D) -> bool {
    let Ok(unk) = t.cast::<IUnknown>() else { return false };
    // SAFETY: one AddRef and its Release on a live object, to read the count: ours, plus `unk`.
    let count = unsafe {
        let vtable = unk.vtable();
        (vtable.AddRef)(unk.as_raw());
        (vtable.Release)(unk.as_raw())
    };
    count <= 2
}

/// The hardware H.264 encoder on `device`'s graphics card, else any hardware one.
fn hardware_h264(device: &ID3D11Device) -> Result<IMFTransform> {
    let output = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_H264 };
    // SAFETY: MF calls with local arguments; the activate array is freed after use.
    unsafe {
        let luid = device.cast::<IDXGIDevice>()?.GetAdapter()?.GetDesc()?.AdapterLuid;
        let mut attrs: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attrs, 1)?;
        let attrs = attrs.ok_or_else(Error::empty)?;
        let bytes = std::slice::from_raw_parts((&raw const luid).cast::<u8>(), size_of_val(&luid));
        attrs.SetBlob(&MFT_ENUM_ADAPTER_LUID, bytes)?;
        let (mut list, mut count) = (std::ptr::null_mut::<Option<IMFActivate>>(), 0u32);
        MFTEnum2(MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER, None, Some(&output), &attrs, &mut list, &mut count)?;
        let found = if list.is_null() { Vec::new() } else { std::slice::from_raw_parts_mut(list, count as usize).iter_mut().map(Option::take).collect::<Vec<_>>() };
        if !list.is_null() {
            CoTaskMemFree(Some(list.cast()));
        }
        let first = found.into_iter().flatten().next().ok_or_else(|| Error::new(windows::Win32::Foundation::E_NOTIMPL, "no hardware H.264 encoder"))?;
        first.ActivateObject::<IMFTransform>()
    }
}

/// A compressed output sample's time, keyframe flag and bytes.
fn read_sample(sample: &IMFSample) -> Result<Encoded> {
    // SAFETY: reads the sample's attributes and locks its one contiguous buffer to copy it.
    unsafe {
        let time = sample.GetSampleTime()?;
        let key = sample.GetUINT32(&MFSampleExtension_CleanPoint).unwrap_or(0) != 0;
        let buffer = sample.ConvertToContiguousBuffer()?;
        let (mut data, mut len) = (std::ptr::null_mut(), 0u32);
        buffer.Lock(&mut data, None, Some(&mut len))?;
        let bytes = std::slice::from_raw_parts(data, len as usize).to_vec();
        buffer.Unlock()?;
        Ok(Encoded { time, key, data: bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Direct3D11::{D3D11_RENDER_TARGET_VIEW_DESC, ID3D11RenderTargetView};

    #[test]
    fn letterbox() {
        let r = fit(1920, 1080);
        assert_eq!((r.left, r.top, r.right, r.bottom), (0, 0, 1920, 1080));
        let r = fit(800, 600);
        assert_eq!((r.left, r.top, r.right, r.bottom), (240, 0, 1680, 1080));
        let r = fit(320, 100);
        assert_eq!((r.left, r.right), (0, 1920));
        assert!(r.top > 0 && r.bottom - r.top == 600);
    }

    /// Step 1 (b, c): 3 s of coloured BGRA frames through the video processor and the hardware
    /// encoder, written as an MP4 with nothing re-encoded, then read back.
    /// `cargo test clips::video -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn encodes_and_muxes() {
        // SAFETY: COM and MF for this test thread.
        unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED).ok().ok();
            windows::Win32::Media::MediaFoundation::MFStartup(windows::Win32::Media::MediaFoundation::MF_VERSION, 0).expect("MFStartup");
        }
        let mut enc = Encoder::new(gpu().expect("device")).expect("a hardware H.264 encoder");
        let source = texture(enc.device(), 1280, 720, DXGI_FORMAT_B8G8R8A8_UNORM).expect("source");
        let mut rtv: Option<ID3D11RenderTargetView> = None;
        // SAFETY: a render-target view on our own texture.
        unsafe { enc.device().CreateRenderTargetView(&source, None::<*const D3D11_RENDER_TARGET_VIEW_DESC>, Some(&mut rtv)).expect("rtv") };
        let rtv = rtv.expect("rtv");
        let mut got = Vec::new();
        let (mut pushed, mut frame) = (0, 0i64);
        let start = std::time::Instant::now();
        while frame < 180 {
            enc.poll(&mut got).expect("poll");
            let shade = (frame % 60) as f32 / 60.0;
            // SAFETY: clears our own render target.
            unsafe { enc.gpu.context.ClearRenderTargetView(&rtv, &[shade, 0.3, 1.0 - shade, 1.0]) };
            if enc.push(&source, frame * FRAME).expect("push") {
                pushed += 1;
                frame += 1;
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert!(start.elapsed().as_secs() < 30, "encoder stalled");
        }
        let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while std::time::Instant::now() < until {
            enc.poll(&mut got).expect("poll");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        println!("pushed {pushed}, got {} samples, {} keyframes, first key: {}", got.len(), got.iter().filter(|e| e.key).count(), got.first().is_some_and(|e| e.key));
        assert!(got.len() >= 170 && got[0].key, "the encoder gives one sample per frame, starting with a keyframe");
        assert!(got.iter().filter(|e| e.key).count() >= 2, "a keyframe every 2 s");
        let path = std::env::temp_dir().join(format!("roalt-clip-test-{}.mp4", uuid::Uuid::new_v4()));
        let samples: Vec<_> = got.iter().map(|e| super::super::mux::Sample { time: e.time, duration: FRAME, key: e.key, data: e.data.clone().into() }).collect();
        let header = super::super::mux::sequence_header(&samples[..1]);
        assert!(header.is_some(), "the first keyframe carries SPS/PPS");
        let video = super::super::mux::Track { kind: enc.out_type().to_vec(), samples: samples.clone(), header: header.clone() };
        super::super::mux::write(&path, &video, None).expect("mp4 written");

        // A clip from the second keyframe on: the stream's own SPS/PPS, carried by the ring.
        let second = samples.iter().enumerate().filter(|(_, s)| s.key).nth(1).map(|(i, _)| i).expect("a second keyframe");
        println!("second keyframe carries SPS/PPS itself: {}", super::super::mux::sequence_header(&samples[second..=second]).is_some());
        let later = super::super::mux::Track {
            kind: enc.out_type().to_vec(),
            samples: samples[second..].iter().map(|s| super::super::mux::Sample { time: s.time - samples[second].time, ..s.clone() }).collect(),
            header,
        };
        let later_path = path.with_extension("later.mp4");
        super::super::mux::write(&later_path, &later, None).expect("later mp4 written");
        let (later_frames, _, later_luma) = super::super::mux::read_back(&later_path, SECOND / 2).expect("later mp4 read back");
        println!("from the second keyframe: {later_frames} frames, centre brightness {later_luma}");
        assert!(later_frames >= 50 && later_luma > 40, "a clip starting at a later keyframe decodes");
        let (frames, last, luma) = super::super::mux::read_back(&path, SECOND / 2).expect("mp4 read back");
        println!(
            "wrote {} ({} bytes): {frames} video frames back, last at {:.2} s, centre brightness at 0.5 s: {luma} (16 is black)",
            path.display(),
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            last as f64 / SECOND as f64
        );
        assert!(frames >= 170 && last > 2 * SECOND, "the MP4 plays back every frame");
        // At 0.5 s the source was cleared to (R .5, G .3, B .5), about Y 100: not black.
        assert!(luma > 40, "the picture made it through the video processor");
    }
}
