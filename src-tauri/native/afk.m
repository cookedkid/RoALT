#import <Cocoa/Cocoa.h>
#import <AVFoundation/AVFoundation.h>
#import <ApplicationServices/ApplicationServices.h>
#include <Carbon/Carbon.h>
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
// macOS virtual key code, not Windows' F13 scan code (0x64).
static CGEventRef f13_event(bool down) {
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStatePrivate);
    if (!source) return NULL;
    // Do not suppress real keyboard/mouse input after our synthetic press.
    CGEventSourceSetLocalEventsSuppressionInterval(source, 0);
    CGEventRef event = CGEventCreateKeyboardEvent(source, kVK_F13, down);
    CFRelease(source);
    if (event) {
        CGEventSetFlags(event, 0);
        CGEventSetIntegerValueField(event, kCGKeyboardEventAutorepeat, 0);
        CGEventSetIntegerValueField(event, kCGEventSourceUserData, marker);
    }
    return event;
}

static bool uninterrupted(uint64_t before) {
    return atomic_load(&last_input) == before && CGEventTapIsEnabled(input_tap);
}

static bool focused(int pid, AXUIElementRef root, AXUIElementRef window) {
    if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != pid) return false;
    CFTypeRef current = NULL;
    AXUIElementCopyAttributeValue(root, kAXFocusedWindowAttribute, &current);
    bool matches = current && CFEqual(current, window);
    if (current) CFRelease(current);
    return matches;
}

// Return the main standard window, or a minimized standard window when there is no main one.
// Dialogs must not receive the anti-AFK key.
static AXUIElementRef game_window(AXUIElementRef root) {
    CFTypeRef main = NULL, windows = NULL;
    AXUIElementCopyAttributeValue(root, kAXMainWindowAttribute, &main);
    AXUIElementCopyAttributeValue(root, kAXWindowsAttribute, &windows);
    AXUIElementRef chosen = NULL;
    if (windows && CFGetTypeID(windows) == CFArrayGetTypeID()) {
        for (CFIndex i = 0; i < CFArrayGetCount(windows); i++) {
            AXUIElementRef window = (AXUIElementRef)CFArrayGetValueAtIndex(windows, i);
            CFTypeRef subrole = NULL;
            AXUIElementCopyAttributeValue(window, kAXSubroleAttribute, &subrole);
            bool standard = subrole && CFEqual(subrole, kAXStandardWindowSubrole);
            if (subrole) CFRelease(subrole);
            if (!standard) continue;
            if (!chosen) chosen = window;
            if (main && CFEqual(main, window)) { chosen = window; break; }
        }
        if (chosen) CFRetain(chosen);
    }
    if (main) CFRelease(main);
    if (windows) CFRelease(windows);
    return chosen;
}

// Match Windows v0.9.2: restore, focus, F13 down/up, restore prior focus and minimized state.
// 0 = posted (not a game acknowledgement), 1 = retry/interrupt, 2 = missing target/window.
static int focus_press(int pid, uint64_t start, uint64_t before) {
    NSRunningApplication *previous = NSWorkspace.sharedWorkspace.frontmostApplication;
    NSRunningApplication *target = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
    if (!target || target.terminated) return 2;
    AXUIElementRef root = AXUIElementCreateApplication(pid);
    AXUIElementSetMessagingTimeout(root, 0.5);
    AXUIElementRef window = game_window(root);
    if (!window) { CFRelease(root); return 2; }
    CFTypeRef mini = NULL;
    AXUIElementCopyAttributeValue(window, kAXMinimizedAttribute, &mini);
    if (!mini || !same(pid, start) || !uninterrupted(before)) {
        if (mini) CFRelease(mini);
        CFRelease(window); CFRelease(root);
        return 1;
    }
    bool was_minimized = CFEqual(mini, kCFBooleanTrue);
    bool was_hidden = target.hidden;
    CFRelease(mini);
    CGEventRef down = f13_event(true), up = f13_event(false);
    if (!down || !up) {
        if (down) CFRelease(down);
        if (up) CFRelease(up);
        CFRelease(window); CFRelease(root);
        return -1;
    }
    // Keep the previous window as well as the app, in case it has several windows.
    CFTypeRef previous_window = NULL;
    if (previous && previous.processIdentifier != pid) {
        AXUIElementRef previous_root = AXUIElementCreateApplication(previous.processIdentifier);
        AXUIElementSetMessagingTimeout(previous_root, 0.5);
        AXUIElementCopyAttributeValue(previous_root, kAXFocusedWindowAttribute, &previous_window);
        CFRelease(previous_root);
    }
    int result = 1;
    bool restored = !was_minimized ||
        AXUIElementSetAttributeValue(window, kAXMinimizedAttribute, kCFBooleanFalse) == kAXErrorSuccess;
    if (restored) {
        if (was_minimized) usleep(300000);
        if (same(pid, start) && uninterrupted(before)) {
            [target activateWithOptions:NSApplicationActivateIgnoringOtherApps];
            AXUIElementPerformAction(window, kAXRaiseAction);
            // Activation is asynchronous; never send a key just because activate returned YES.
            for (int i = 0; i < 10 && uninterrupted(before) && same(pid, start); i++) {
                if (focused(pid, root, window)) break;
                usleep(50000);
            }
            if (same(pid, start) && uninterrupted(before) && focused(pid, root, window)) {
                // The foreground event stream is the Mac equivalent of SendInput. Posting
                // directly to a background PID is deliberately not used for key-down.
                CGEventPost(kCGHIDEventTap, down);
                usleep(40000);
                bool kept_focus = focused(pid, root, window);
                // Always balance a posted down, even if the user interrupted or focus moved.
                CGEventPost(kCGHIDEventTap, up);
                usleep(40000);
                if (kept_focus && uninterrupted(before) && focused(pid, root, window)) result = 0;
            }
        }
        // If a person returns, leave their chosen focus/window state alone and stop the round.
        if (same(pid, start) && uninterrupted(before)) {
            bool front = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == pid;
            if (was_minimized && AXUIElementSetAttributeValue(window, kAXMinimizedAttribute, kCFBooleanTrue) != kAXErrorSuccess)
                result = 1;
            if (was_hidden && ![target hide]) result = 1;
            if (front && previous && previous.processIdentifier != pid && !previous.terminated) {
                [previous activateWithOptions:NSApplicationActivateIgnoringOtherApps];
                if (previous_window) AXUIElementPerformAction((AXUIElementRef)previous_window, kAXRaiseAction);
                for (int i = 0; i < 10 && uninterrupted(before); i++) {
                    if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == previous.processIdentifier) break;
                    usleep(50000);
                }
                if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != previous.processIdentifier) result = 1;
            }
        } else result = 1;
    }
    if (previous_window) CFRelease(previous_window);
    CFRelease(down); CFRelease(up); CFRelease(window); CFRelease(root);
    return result;
}

int ro_afk_press(int pid, uint64_t start) {
    @autoreleasepool {
        if (!same(pid, start)) return 2;
        if (ro_afk_ready() != 0) return -2;
        uint64_t before = atomic_load(&last_input);
        if (now_ns() - before < 5000000000ULL || !uninterrupted(before)) return 1;
        if (!same(pid, start)) return 2;
        return focus_press(pid, start, before);
    }
}
