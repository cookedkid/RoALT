#import <Cocoa/Cocoa.h>
#import <AVFoundation/AVFoundation.h>
#import <ApplicationServices/ApplicationServices.h>
#include <libproc.h>
#include <stdatomic.h>
#include <time.h>
#include <pthread.h>
// Retry Accessibility at startup when the current signed build is not trusted.
// TCC grants belong to code identity; an older build's enabled entry can be stale.
void ro_request_accessibility(void) {
    if (AXIsProcessTrusted()) return;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        dispatch_async(dispatch_get_main_queue(), ^{
            NSDictionary *options = @{(__bridge id)kAXTrustedCheckOptionPrompt: @YES};
            AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options);
        });
    });
}
void ro_request_microphone_if_needed(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        // A bare development executable has no purpose string. TCC aborts such requests.
        NSString *purpose = [NSBundle.mainBundle objectForInfoDictionaryKey:@"NSMicrophoneUsageDescription"];
        if (![purpose isKindOfClass:NSString.class] || purpose.length == 0) return;
        if ([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio] == AVAuthorizationStatusNotDetermined) {
            [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL granted) {
                // Voice chat is optional. Launching Roblox needs no permission result.
                (void)granted;
            }];
        }
    });
}

void ro_request_initial_permissions(void) {
    @autoreleasepool {
        if (!CGPreflightScreenCaptureAccess()) CGRequestScreenCaptureAccess();
        if (!CGPreflightListenEventAccess()) CGRequestListenEventAccess();
        if (!CGPreflightPostEventAccess()) CGRequestPostEventAccess();
        ro_request_accessibility();
    }
}
static _Atomic uint64_t last_input=0;
static CFMachPortRef input_tap=NULL;
static pthread_mutex_t setup_lock=PTHREAD_MUTEX_INITIALIZER;
static const int64_t marker=0x524f414c5441464bLL;
static uint64_t now_ns(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (uint64_t)t.tv_sec*1000000000+t.tv_nsec;}
static CGEventRef observe(CGEventTapProxy proxy,CGEventType type,CGEventRef event,void *unused){
 if(type==kCGEventTapDisabledByTimeout||type==kCGEventTapDisabledByUserInput){atomic_store(&last_input,now_ns());if(input_tap)CGEventTapEnable(input_tap,true);return event;}
 if(CGEventGetIntegerValueField(event,kCGEventSourceUserData)!=marker||CGEventGetIntegerValueField(event,kCGEventSourceUnixProcessID)!=getpid())atomic_store(&last_input,now_ns());return event;
}
static void *listen_input(void *unused){CFRunLoopSourceRef source=CFMachPortCreateRunLoopSource(NULL,input_tap,0);CFRunLoopAddSource(CFRunLoopGetCurrent(),source,kCFRunLoopCommonModes);CGEventTapEnable(input_tap,true);CFRunLoopRun();CFRelease(source);return NULL;}
int ro_afk_ready(void){
 if(!AXIsProcessTrusted()||!CGPreflightPostEventAccess()||!CGPreflightListenEventAccess())return -2;
 pthread_mutex_lock(&setup_lock);
 if(!input_tap){CGEventMask mask=CGEventMaskBit(kCGEventKeyDown)|CGEventMaskBit(kCGEventKeyUp)|CGEventMaskBit(kCGEventFlagsChanged)|CGEventMaskBit(kCGEventMouseMoved)|CGEventMaskBit(kCGEventLeftMouseDown)|CGEventMaskBit(kCGEventRightMouseDown)|CGEventMaskBit(kCGEventOtherMouseDown)|CGEventMaskBit(kCGEventScrollWheel)|CGEventMaskBit(kCGEventLeftMouseUp)|CGEventMaskBit(kCGEventRightMouseUp)|CGEventMaskBit(kCGEventOtherMouseUp)|CGEventMaskBit(kCGEventOtherMouseDragged)|CGEventMaskBit(kCGEventLeftMouseDragged)|CGEventMaskBit(kCGEventRightMouseDragged);input_tap=CGEventTapCreate(kCGSessionEventTap,kCGHeadInsertEventTap,kCGEventTapOptionListenOnly,mask,observe,NULL);if(input_tap){atomic_store(&last_input,now_ns());pthread_t thread;if(pthread_create(&thread,NULL,listen_input,NULL)){CFRelease(input_tap);input_tap=NULL;}else pthread_detach(thread);}}
 int result=input_tap?0:-2;pthread_mutex_unlock(&setup_lock);return result;
}
static BOOL same(int pid,uint64_t start){struct proc_bsdinfo b={0};return proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&b,sizeof(b))==sizeof(b)&&b.pbi_uid==getuid()&&b.pbi_start_tvsec==start;}
static bool window_point(AXUIElementRef window, CGPoint *center) {
 CFTypeRef position=NULL, size=NULL;
 AXUIElementCopyAttributeValue(window,kAXPositionAttribute,&position);
 AXUIElementCopyAttributeValue(window,kAXSizeAttribute,&size);
 CGPoint origin; CGSize extent;
 bool ok=position && size && CFGetTypeID(position)==AXValueGetTypeID() &&
     CFGetTypeID(size)==AXValueGetTypeID() && AXValueGetValue(position,kAXValueCGPointType,&origin) &&
     AXValueGetValue(size,kAXValueCGSizeType,&extent) && extent.width>0 && extent.height>60;
 if(ok) *center=CGPointMake(origin.x+extent.width/2,origin.y+extent.height/2);
 if(position) CFRelease(position);
 if(size) CFRelease(size);
 return ok;
}

static int focus_click(int pid,uint64_t start,uint64_t before) {
 NSRunningApplication *previous=NSWorkspace.sharedWorkspace.frontmostApplication;
 NSRunningApplication *target=[NSRunningApplication runningApplicationWithProcessIdentifier:pid];
 AXUIElementRef root=AXUIElementCreateApplication(pid);
 AXUIElementSetMessagingTimeout(root,0.5);
 CFTypeRef windows=NULL;
 AXUIElementCopyAttributeValue(root,kAXWindowsAttribute,&windows);
 CFRelease(root);
 if(!windows) return 2;
 int result=2;
 for(CFIndex i=0;i<CFArrayGetCount(windows);i++){
  AXUIElementRef window=(AXUIElementRef)CFArrayGetValueAtIndex(windows,i);
  CGPoint center;
  if(!window_point(window,&center)) continue;
  if(!same(pid,start)||atomic_load(&last_input)!=before) break;
  CFTypeRef mini=NULL;
  AXUIElementCopyAttributeValue(window,kAXMinimizedAttribute,&mini);
  if(!mini) continue;
  bool was_minimized=CFEqual(mini,kCFBooleanTrue);
  CFRelease(mini);
  if(was_minimized && AXUIElementSetAttributeValue(window,kAXMinimizedAttribute,kCFBooleanFalse)!=kAXErrorSuccess) continue;
  [target activateWithOptions:NSApplicationActivateIgnoringOtherApps];
  AXUIElementPerformAction(window,kAXRaiseAction);
  usleep(150000);
  result=1;
  if(same(pid,start) && atomic_load(&last_input)==before &&
     NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier==pid && window_point(window,&center)){
   CGEventRef down=CGEventCreateMouseEvent(NULL,kCGEventLeftMouseDown,center,kCGMouseButtonLeft);
   CGEventRef up=CGEventCreateMouseEvent(NULL,kCGEventLeftMouseUp,center,kCGMouseButtonLeft);
   result=-1;
   if(down && up){
    CGEventSetIntegerValueField(down,kCGEventSourceUserData,marker);
    CGEventSetIntegerValueField(up,kCGEventSourceUserData,marker);
    CGEventPostToPid(pid,down);
    CGEventPostToPid(pid,up);
    usleep(1000000);
    result=0;
   }
   if(down) CFRelease(down);
   if(up) CFRelease(up);
  }
  if(atomic_load(&last_input)==before && same(pid,start)){
   bool front=NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier==pid;
   if((result==0 && front)||was_minimized)
    AXUIElementSetAttributeValue(window,kAXMinimizedAttribute,kCFBooleanTrue);
   if(front && previous.processIdentifier!=pid) [previous activateWithOptions:NSApplicationActivateIgnoringOtherApps];
  }
  break;
 }
 CFRelease(windows);
 return result;
}

int ro_afk_click(int pid,uint64_t start){@autoreleasepool{
 if(!same(pid,start))return 2;if(ro_afk_ready()!=0)return -2;uint64_t before=atomic_load(&last_input);if(now_ns()-before<5000000000ULL||!CGEventTapIsEnabled(input_tap))return 1;if(!same(pid,start))return 2;
 return focus_click(pid,start,before);
}}
