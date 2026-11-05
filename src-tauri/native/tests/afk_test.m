// Compile with Cocoa, ApplicationServices and AVFoundation; never posts input or focuses apps.
#import "../afk.m"
#include <assert.h>
int main(void) {
    @autoreleasepool {
        CGEventRef down = f13_event(true), up = f13_event(false);
        assert(down && up);
        assert(CGEventGetType(down) == kCGEventKeyDown);
        assert(CGEventGetType(up) == kCGEventKeyUp);
        assert(CGEventGetIntegerValueField(down, kCGKeyboardEventKeycode) == 105);
        assert(CGEventGetIntegerValueField(up, kCGKeyboardEventKeycode) == 105);
        assert(CGEventGetFlags(down) == 0 && CGEventGetFlags(up) == 0);
        assert(CGEventGetIntegerValueField(down, kCGKeyboardEventAutorepeat) == 0);
        assert(CGEventGetIntegerValueField(down, kCGEventSourceUserData) == marker);
        // Our own tagged events must not interrupt the rest of a multi-client round.
        assert(CGEventGetIntegerValueField(down, kCGEventSourceUnixProcessID) == getpid());
        atomic_store(&last_input, 123);
        observe(NULL, kCGEventKeyDown, down, NULL);
        assert(atomic_load(&last_input) == 123);
        // Real input (or another tool's input) must interrupt it.
        CGEventRef human = CGEventCreateKeyboardEvent(NULL, 0, true);
        assert(human);
        observe(NULL, kCGEventKeyDown, human, NULL);
        CFRelease(human);
        assert(atomic_load(&last_input) != 123);
        // PID reuse / wrong birth time must fail before permission checks or window changes.
        assert(ro_afk_press(getpid(), 1) == 2);
        CFRelease(down); CFRelease(up);
        puts("anti-AFK native checks passed (no input posted)");
    }
}
