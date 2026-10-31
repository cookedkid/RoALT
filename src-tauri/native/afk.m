#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#include <libproc.h>
#include <stdatomic.h>
#include <time.h>
#include <pthread.h>
// macOS presents its own approval dialogs. Request each permission only on first run.
void ro_request_initial_permissions(void) {
    @autoreleasepool {
        if (!CGPreflightScreenCaptureAccess()) CGRequestScreenCaptureAccess();
        if (!CGPreflightListenEventAccess()) CGRequestListenEventAccess();
        if (!CGPreflightPostEventAccess()) CGRequestPostEventAccess();
        // Accessibility opens System Settings asynchronously, so ask for it last.
        if (!AXIsProcessTrusted()) {
            NSDictionary *options = @{(__bridge id)kAXTrustedCheckOptionPrompt: @YES};
            AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options);
        }
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
int ro_afk_press_mode(int pid,uint64_t start,bool background,int keycode){@autoreleasepool{
 if(keycode!=48&&keycode!=49)return -3;
 if(!same(pid,start))return 2;if(ro_afk_ready()!=0)return -2;uint64_t before=atomic_load(&last_input);if(now_ns()-before<5000000000ULL||!CGEventTapIsEnabled(input_tap))return 1;if(!same(pid,start))return 2;
 NSRunningApplication *previous=nil;
 if(!background){
 AXUIElementRef root=AXUIElementCreateApplication(pid);AXUIElementSetMessagingTimeout(root,0.5);CFTypeRef windows=NULL;AXUIElementCopyAttributeValue(root,kAXWindowsAttribute,&windows);CFRelease(root);if(!windows)return 2;
 BOOL visible=NO;for(CFIndex i=0;i<CFArrayGetCount(windows);i++){CFTypeRef mini=NULL;if(AXUIElementCopyAttributeValue((AXUIElementRef)CFArrayGetValueAtIndex(windows,i),kAXMinimizedAttribute,&mini)==kAXErrorSuccess&&mini){visible=!CFBooleanGetValue(mini);CFRelease(mini);if(visible)break;}}CFRelease(windows);if(!visible)return 2;
 previous=NSWorkspace.sharedWorkspace.frontmostApplication;NSRunningApplication *target=[NSRunningApplication runningApplicationWithProcessIdentifier:pid];if(!target||target.hidden)return 2;if(atomic_load(&last_input)!=before||![target activateWithOptions:NSApplicationActivateIgnoringOtherApps])return 1;
 usleep(50000);if(atomic_load(&last_input)!=before||NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier!=pid||!same(pid,start))return 1;
 }
 CGKeyCode key=(CGKeyCode)keycode;
 CGEventSourceRef source=CGEventSourceCreate(kCGEventSourceStatePrivate);if(!source)return -1;CGEventRef down=CGEventCreateKeyboardEvent(source,key,true),up=CGEventCreateKeyboardEvent(source,key,false);if(source)CFRelease(source);if(!down||!up){if(down)CFRelease(down);if(up)CFRelease(up);return -1;}
 CGEventSetIntegerValueField(down,kCGEventSourceUserData,marker);CGEventSetIntegerValueField(up,kCGEventSourceUserData,marker);
 // Both edges target the verified client PID, never whichever app happens to be frontmost.
 if(atomic_load(&last_input)!=before||!same(pid,start)){CFRelease(down);CFRelease(up);return 1;}
 CGEventPostToPid(pid,down);usleep(20000);if(same(pid,start))CGEventPostToPid(pid,up);CFRelease(down);CFRelease(up);
 BOOL interrupted=atomic_load(&last_input)!=before;if(!background&&!interrupted&&NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier==pid)[previous activateWithOptions:NSApplicationActivateIgnoringOtherApps];return interrupted?1:0;
}}
