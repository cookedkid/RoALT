#import <Cocoa/Cocoa.h>
#import <Security/Security.h>
#include <sys/stat.h>

static BOOL ownedDirectory(NSURL *url) {
    struct stat info;
    return lstat(url.fileSystemRepresentation, &info) == 0 &&
           S_ISDIR(info.st_mode) && info.st_uid == getuid();
}

static BOOL releaseName(NSString *name) {
    return [name rangeOfString:@"^RoALT( [0-9]+| \\([0-9]+\\))?\\.app$"
                       options:NSRegularExpressionSearch].location != NSNotFound;
}

static NSData *signedIdentity(NSURL *url) {
    SecStaticCodeRef code = NULL;
    SecRequirementRef requirement = NULL;
    CFDictionaryRef info = NULL;
    NSData *hash = nil;
    if (SecStaticCodeCreateWithPath((__bridge CFURLRef)url, kSecCSDefaultFlags, &code) == errSecSuccess &&
        SecRequirementCreateWithString(CFSTR("identifier \"com.inspectsoftware.roalt\""), 0, &requirement) == errSecSuccess &&
        SecStaticCodeCheckValidity(code, kSecCSStrictValidate, requirement) == errSecSuccess &&
        SecCodeCopySigningInformation(code, kSecCSSigningInformation, &info) == errSecSuccess) {
        hash = ((__bridge NSDictionary *)info)[(__bridge id)kSecCodeInfoUnique];
    }
    if (info) CFRelease(info);
    if (requirement) CFRelease(requirement);
    if (code) CFRelease(code);
    return hash;
}

void ro_cleanup_installed_duplicates(void) {
    @autoreleasepool {
        NSBundle *installed = NSBundle.mainBundle;
        NSURL *current = installed.bundleURL.URLByResolvingSymlinksInPath;
        NSString *userApps = [NSHomeDirectory() stringByAppendingPathComponent:@"Applications"];
        NSString *downloads = [NSHomeDirectory() stringByAppendingPathComponent:@"Downloads"];
        NSString *installedDir = current.URLByDeletingLastPathComponent.path;
        if (![current.lastPathComponent isEqual:@"RoALT.app"] ||
            !([installedDir isEqual:userApps] || [installedDir isEqual:@"/Applications"])) return;
        NSString *version = installed.infoDictionary[@"CFBundleShortVersionString"];
        NSData *currentHash = signedIdentity(current);
        if (!currentHash || !version) return;
        NSFileManager *fm = NSFileManager.defaultManager;
        for (NSString *dir in @[downloads, userApps, @"/Applications"]) {
            NSURL *folder = [NSURL fileURLWithPath:dir isDirectory:YES];
            if (![folder.URLByResolvingSymlinksInPath.path isEqual:dir]) continue;
            for (NSURL *url in [fm contentsOfDirectoryAtURL:folder includingPropertiesForKeys:nil
                                options:NSDirectoryEnumerationSkipsHiddenFiles error:nil]) {
                if (!releaseName(url.lastPathComponent) || [url.path isEqual:current.path] || !ownedDirectory(url)) continue;
                NSBundle *bundle = [NSBundle bundleWithURL:url];
                NSString *other = bundle.infoDictionary[@"CFBundleShortVersionString"];
                if (![bundle.bundleIdentifier isEqual:installed.bundleIdentifier] ||
                    ![bundle.infoDictionary[@"CFBundleExecutable"] isEqual:@"roalt"] ||
                    ![other isKindOfClass:NSString.class] ||
                    [other rangeOfString:@"^[0-9]+\\.[0-9]+\\.[0-9]+$" options:NSRegularExpressionSearch].location == NSNotFound) continue;
                NSComparisonResult order = [other compare:version options:NSNumericSearch];
                if (order == NSOrderedDescending) continue;
                NSData *hash = signedIdentity(url);
                if (!hash || (order == NSOrderedSame && ![hash isEqual:currentHash])) continue;
                BOOL running = NO;
                for (NSRunningApplication *app in [NSRunningApplication runningApplicationsWithBundleIdentifier:installed.bundleIdentifier]) {
                    if ([app.bundleURL.URLByResolvingSymlinksInPath.path isEqual:url.path]) { running = YES; break; }
                }
                if (!running) {
                    NSError *error = nil;
                    if (![fm trashItemAtURL:url resultingItemURL:nil error:&error])
                        NSLog(@"RoALT could not move duplicate %@ to Trash: %@", url.lastPathComponent, error.localizedDescription);
                }
            }
        }
    }
}
