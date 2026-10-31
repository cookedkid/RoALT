#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#include <libproc.h>
#include <dlfcn.h>
struct RoRect {int x,y,w,h;};
static BOOL identity(int pid,uint64_t start){struct proc_bsdinfo b={0};return proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&b,sizeof(b))==sizeof(b)&&b.pbi_start_tvsec==start&&b.pbi_uid==getuid();}
static AXUIElementRef window(int pid){AXUIElementRef root=AXUIElementCreateApplication(pid);AXUIElementSetMessagingTimeout(root,0.5);CFTypeRef list=NULL;AXUIElementCopyAttributeValue(root,kAXWindowsAttribute,&list);CFRelease(root);if(!list)return NULL;AXUIElementRef w=CFArrayGetCount(list)?(AXUIElementRef)CFRetain(CFArrayGetValueAtIndex(list,0)):NULL;CFRelease(list);if(w)AXUIElementSetMessagingTimeout(w,0.5);return w;}
int ro_rect(int pid,uint64_t start,struct RoRect *r,int set){
 if(!identity(pid,start))return -1;if(!AXIsProcessTrusted())return -2;
 AXUIElementRef w=window(pid);if(!w)return -1;CFTypeRef mini=NULL;AXUIElementCopyAttributeValue(w,kAXMinimizedAttribute,&mini);BOOL minimized=mini&&CFBooleanGetValue(mini);if(mini)CFRelease(mini);if(minimized){CFRelease(w);return 1;}
 AXError err=kAXErrorSuccess;
 CFTypeRef oldp=NULL,olds=NULL;if(set){AXUIElementCopyAttributeValue(w,kAXPositionAttribute,&oldp);AXUIElementCopyAttributeValue(w,kAXSizeAttribute,&olds);if(!oldp||!olds){if(oldp)CFRelease(oldp);if(olds)CFRelease(olds);CFRelease(w);return -1;}}
 if(set){CGPoint p=CGPointMake(r->x,r->y);CGSize s=CGSizeMake(r->w,r->h);AXValueRef pv=AXValueCreate(kAXValueCGPointType,&p),sv=AXValueCreate(kAXValueCGSizeType,&s);if(identity(pid,start)){err=AXUIElementSetAttributeValue(w,kAXPositionAttribute,pv);if(!err)err=AXUIElementSetAttributeValue(w,kAXSizeAttribute,sv);}else err=kAXErrorFailure;CFRelease(pv);CFRelease(sv);}
 CFTypeRef p=NULL,s=NULL;CGPoint point={0};CGSize size={0};if(!err)err=AXUIElementCopyAttributeValue(w,kAXPositionAttribute,&p);if(!err)err=AXUIElementCopyAttributeValue(w,kAXSizeAttribute,&s);
 if(p&&s){AXValueGetValue(p,kAXValueCGPointType,&point);AXValueGetValue(s,kAXValueCGSizeType,&size);*r=(struct RoRect){point.x,point.y,size.width,size.height};}else err=kAXErrorFailure;
 if(set&&err&&identity(pid,start)){AXUIElementSetAttributeValue(w,kAXPositionAttribute,oldp);AXUIElementSetAttributeValue(w,kAXSizeAttribute,olds);}
 if(oldp)CFRelease(oldp);if(olds)CFRelease(olds);if(p)CFRelease(p);if(s)CFRelease(s);CFRelease(w);return !err&&identity(pid,start)?0:-1;
}
int ro_workarea(struct RoRect *r){@autoreleasepool{__block NSRect area=NSZeroRect;void(^read)(void)=^{NSScreen *screen=NSScreen.mainScreen;NSRect a=screen.visibleFrame;CGFloat top=NSScreen.screens.firstObject.frame.size.height;area=NSMakeRect(a.origin.x,top-NSMaxY(a),a.size.width,a.size.height);};if(NSThread.isMainThread)read();else dispatch_sync(dispatch_get_main_queue(),read);*r=(struct RoRect){area.origin.x,area.origin.y,area.size.width,area.size.height};return r->w>0?0:-1;}}
@interface RoCapture:NSObject
@property NSLock *lock;
@property dispatch_semaphore_t ready;
@property BOOL cancelled;
@property CGImageRef image;
@end
@implementation RoCapture
-(instancetype)init{self=[super init];if(self){_lock=[NSLock new];_ready=dispatch_semaphore_create(0);}return self;}
-(void)dealloc{if(_image)CGImageRelease(_image);}
@end
// Only a named window belonging to the tracked client is ever captured.
int ro_capture(int pid,uint64_t start,int maxwidth,unsigned char **bytes,int *width,int *height){@autoreleasepool{
 if(!identity(pid,start))return -1;if(!CGPreflightScreenCaptureAccess())return -3;
 NSArray *windows=CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly|kCGWindowListExcludeDesktopElements,kCGNullWindowID));
 CGWindowID wid=0;double best=0;for(NSDictionary *w in windows){if([w[(id)kCGWindowOwnerPID]intValue]!=pid||[w[(id)kCGWindowLayer]intValue]!=0)continue;CGRect b;CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(id)kCGWindowBounds],&b);double area=b.size.width*b.size.height;if(area>best){best=area;wid=[w[(id)kCGWindowNumber]unsignedIntValue];}}
 if(!wid)return 1;RoCapture *request=[RoCapture new];
 if(@available(macOS 14.0,*)){
  [SCShareableContent getShareableContentExcludingDesktopWindows:YES onScreenWindowsOnly:YES completionHandler:^(SCShareableContent *content,NSError *error){
   SCWindow *target=nil;for(SCWindow *w in content.windows)if(w.windowID==wid&&w.owningApplication.processID==pid){target=w;break;}
   [request.lock lock];BOOL cancelled=request.cancelled;[request.lock unlock];
   if(!target||cancelled||!identity(pid,start)){dispatch_semaphore_signal(request.ready);return;}
   SCContentFilter *filter=[[SCContentFilter alloc]initWithDesktopIndependentWindow:target];SCStreamConfiguration *config=[SCStreamConfiguration new];config.width=MIN(maxwidth,target.frame.size.width);config.height=MAX(1,round(config.width*target.frame.size.height/MAX(1,target.frame.size.width)));config.showsCursor=NO;
   [SCScreenshotManager captureImageWithFilter:filter configuration:config completionHandler:^(CGImageRef image,NSError *error){[request.lock lock];if(image&&!request.cancelled)request.image=CGImageRetain(image);[request.lock unlock];dispatch_semaphore_signal(request.ready);}];
  }];
  dispatch_semaphore_wait(request.ready,dispatch_time(DISPATCH_TIME_NOW,2*NSEC_PER_SEC));
 }else{
  typedef CGImageRef(*Capture)(CGRect,CGWindowListOption,CGWindowID,CGWindowImageOption);Capture capture=(Capture)dlsym(RTLD_DEFAULT,"CGWindowListCreateImage");if(capture)request.image=capture(CGRectNull,kCGWindowListOptionIncludingWindow,wid,kCGWindowImageBoundsIgnoreFraming);
 }
 [request.lock lock];request.cancelled=YES;CGImageRef image=request.image?CGImageRetain(request.image):NULL;[request.lock unlock];if(!image)return -4;if(!identity(pid,start)){CGImageRelease(image);return -1;}
 size_t iw=CGImageGetWidth(image),ih=CGImageGetHeight(image);*width=MIN(maxwidth,iw);*height=MAX(1,(int)((double)*width*ih/MAX(1,iw)));if(*height>8192){CGImageRelease(image);return -4;}
 *bytes=calloc((size_t)*width* *height,4);if(!*bytes){CGImageRelease(image);return -4;}CGColorSpaceRef space=CGColorSpaceCreateDeviceRGB();CGContextRef ctx=CGBitmapContextCreate(*bytes,*width,*height,8,*width*4,space,kCGImageAlphaPremultipliedLast|kCGBitmapByteOrder32Big);CGColorSpaceRelease(space);if(!ctx){free(*bytes);*bytes=NULL;CGImageRelease(image);return -4;}
 CGContextDrawImage(ctx,CGRectMake(0,0,*width,*height),image);CGContextRelease(ctx);CGImageRelease(image);return 0;
}}
void ro_capture_free(void *bytes){free(bytes);}
