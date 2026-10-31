// Public macOS APIs only. No Roblox memory access, task ports or executable modification.
#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#include <libproc.h>
#include <sys/resource.h>
#include <sys/sysctl.h>
#include <mach/mach.h>
#include <mach/mach_time.h>
#include <semaphore.h>
#include <errno.h>
#include <stdint.h>

struct RoProcess { uint64_t start, cpu, resident, footprint, read, written; };
int ro_process(int pid, struct RoProcess *out) {
 struct proc_bsdinfo b = {0}; struct rusage_info_v2 r = {0};
 if (proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &b, sizeof(b)) != sizeof(b)) return -1;
 if (proc_pid_rusage(pid, RUSAGE_INFO_V2, (rusage_info_t *)&r)) return -1;
 out->start=b.pbi_start_tvsec;
 // proc_pid_rusage CPU counters use Mach absolute time, not nanoseconds on Apple Silicon.
 mach_timebase_info_data_t timebase={0};mach_timebase_info(&timebase);
 out->cpu=(uint64_t)(((__uint128_t)r.ri_user_time+r.ri_system_time)*timebase.numer/timebase.denom);
 out->resident=r.ri_resident_size;out->footprint=r.ri_phys_footprint;
 out->read=r.ri_diskio_bytesread;out->written=r.ri_diskio_byteswritten; return 0;
}
int ro_background(int pid, int on) { return setpriority(PRIO_DARWIN_PROCESS,pid,on?PRIO_DARWIN_BG:0); }
int ro_thread_priority(int pid) {
 uint64_t threads[512]={0};int bytes=proc_pidinfo(pid,PROC_PIDLISTTHREADS,0,threads,sizeof(threads));
 if(bytes<(int)sizeof(uint64_t))return -1;
 struct proc_threadinfo info={0};
 if(proc_pidinfo(pid,PROC_PIDTHREADINFO,threads[0],&info,sizeof(info))!=sizeof(info))return -1;
 return info.pth_priority;
}
int ro_unlink_singleton(void) { if (sem_unlink("/RobloxPlayerUniq")==0 || errno==ENOENT) return 0; return errno; }
struct RoSystem { uint64_t idle,total,used,memory; };
int ro_system(struct RoSystem *out) {
 host_cpu_load_info_data_t cpu={0}; mach_msg_type_number_t n=HOST_CPU_LOAD_INFO_COUNT;
 mach_port_t host=mach_host_self();
 if(host_statistics(host,HOST_CPU_LOAD_INFO,(host_info_t)&cpu,&n)!=KERN_SUCCESS){mach_port_deallocate(mach_task_self(),host);return -1;}
 out->idle=cpu.cpu_ticks[CPU_STATE_IDLE];out->total=0;
 for(int i=0;i<CPU_STATE_MAX;i++)out->total+=cpu.cpu_ticks[i];
 vm_statistics64_data_t vm={0};n=HOST_VM_INFO64_COUNT;
 if(host_statistics64(host,HOST_VM_INFO64,(host_info64_t)&vm,&n)!=KERN_SUCCESS){mach_port_deallocate(mach_task_self(),host);return -1;}
 vm_size_t page=0;host_page_size(host,&page);mach_port_deallocate(mach_task_self(),host);
 size_t size=sizeof(out->memory); if(sysctlbyname("hw.memsize",&out->memory,&size,NULL,0))return -1;
 uint64_t freebytes=((uint64_t)vm.free_count+vm.inactive_count)*page;
 out->used=out->memory>freebytes?out->memory-freebytes:0;return 0;
}
int ro_foreground(void) { @autoreleasepool { return NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier; } }
int ro_has_window(int pid) {
 @autoreleasepool {
  NSArray *list=CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionAll,kCGNullWindowID));
  for(NSDictionary *w in list)if([w[(id)kCGWindowOwnerPID] intValue]==pid && [w[(id)kCGWindowLayer] intValue]==0)return 1;
  return 0;
 }
}
// 0 focus, 1 minimize, 2 restore, 3 resize. AX operations require user-granted Accessibility.
int ro_window(int pid,int action,int x,int y,int width,int height) {
 @autoreleasepool {
  NSRunningApplication *app=[NSRunningApplication runningApplicationWithProcessIdentifier:pid];
  if(!app)return -1;
  if(action==0)return [app activateWithOptions:NSApplicationActivateIgnoringOtherApps]?0:-1;
  if(!AXIsProcessTrusted())return -2;
  AXUIElementRef root=AXUIElementCreateApplication(pid);CFTypeRef value=NULL;
  AXError err=AXUIElementCopyAttributeValue(root,kAXWindowsAttribute,&value);CFRelease(root);
  if(err!=kAXErrorSuccess || !value)return -1;
  CFArrayRef windows=(CFArrayRef)value;int count=(int)CFArrayGetCount(windows);int result=-1;
  for(int i=0;i<count;i++) {
   AXUIElementRef w=(AXUIElementRef)CFArrayGetValueAtIndex(windows,i);
   if(action==1||action==2)err=AXUIElementSetAttributeValue(w,kAXMinimizedAttribute,action==1?kCFBooleanTrue:kCFBooleanFalse);
   else {
    CGPoint point=CGPointMake(x,y);CGSize size=CGSizeMake(width,height);
    AXValueRef p=AXValueCreate(kAXValueCGPointType,&point),s=AXValueCreate(kAXValueCGSizeType,&size);
    err=AXUIElementSetAttributeValue(w,kAXPositionAttribute,p);
    if(err==kAXErrorSuccess)err=AXUIElementSetAttributeValue(w,kAXSizeAttribute,s);
    CFRelease(p);CFRelease(s);
   }
   if(err==kAXErrorSuccess){result=0;break;}
  }
  CFRelease(value);
  if(action==2&&result==0)[app activateWithOptions:NSApplicationActivateIgnoringOtherApps];
  return result;
 }
}
// Independent POSIX clock for the CPU-unit regression test.
uint64_t ro_self_cpu_ns(void) {
 struct rusage usage={0};if(getrusage(RUSAGE_SELF,&usage))return 0;
 return ((uint64_t)usage.ru_utime.tv_sec+usage.ru_stime.tv_sec)*1000000000ULL
       +((uint64_t)usage.ru_utime.tv_usec+usage.ru_stime.tv_usec)*1000ULL;
}

#import <LocalAuthentication/LocalAuthentication.h>
bool ro_biometrics_available(void) {
    return [[LAContext new] canEvaluatePolicy:LAPolicyDeviceOwnerAuthenticationWithBiometrics error:nil];
}
bool ro_biometrics_verify(const char *reason) {
    @autoreleasepool {
        LAContext *context=[LAContext new];
        context.localizedFallbackTitle=@"";
        dispatch_semaphore_t completed=dispatch_semaphore_create(0);
        __block BOOL verified=NO;
        [context evaluatePolicy:LAPolicyDeviceOwnerAuthenticationWithBiometrics localizedReason:[NSString stringWithUTF8String:reason] reply:^(BOOL success,NSError *error){
            verified=success;
            dispatch_semaphore_signal(completed);
        }];
        long timed_out=dispatch_semaphore_wait(completed,dispatch_time(DISPATCH_TIME_NOW,60*NSEC_PER_SEC));
        [context invalidate];
        return !timed_out && verified;
    }
}
// Only explicit AX responsiveness timeouts count as a missed response. Permission denial,
// unavailable attributes and disappearing windows cannot qualify a client for recovery.
int ro_responsiveness(int pid) {
    if(!AXIsProcessTrusted()) return 0;
    AXUIElementRef app=AXUIElementCreateApplication(pid);
    AXUIElementSetMessagingTimeout(app,0.75);
    CFTypeRef role=NULL;
    double before=CFAbsoluteTimeGetCurrent();
    AXError err=AXUIElementCopyAttributeValue(app,kAXRoleAttribute,&role);
    double elapsed=CFAbsoluteTimeGetCurrent()-before;
    if(role) CFRelease(role);
    CFRelease(app);
    if(err==kAXErrorSuccess) return 1;
    return err==kAXErrorCannotComplete && elapsed>=0.7 ? -1:0;
}

bool ro_preferred_language(char *out,size_t len) {
    return [NSLocale.preferredLanguages.firstObject getCString:out maxLength:len encoding:NSUTF8StringEncoding];
}
#import <IOKit/IOKitLib.h>
double ro_gpu_pct(void) {
    @autoreleasepool {
        io_iterator_t services=0;
        if(IOServiceGetMatchingServices(kIOMasterPortDefault,IOServiceMatching("IOAccelerator"),&services)!=KERN_SUCCESS) return -1;
        double utilization=-1;
        io_service_t service;
        while((service=IOIteratorNext(services))) {
            NSDictionary *stats=CFBridgingRelease(IORegistryEntryCreateCFProperty(service,CFSTR("PerformanceStatistics"),NULL,0));
            NSNumber *value=[stats isKindOfClass:NSDictionary.class]?stats[@"Device Utilization %"]:nil;
            if([value isKindOfClass:NSNumber.class] && value.doubleValue>=0 && value.doubleValue<=100)
                utilization=MAX(utilization,value.doubleValue);
            IOObjectRelease(service);
        }
        IOObjectRelease(services);
        return utilization;
    }
}

// Exact PID-owned log binding: Mac Roblox may omit the browser tracker ID.
int ro_open_log(int pid, uint64_t start, const char *directory, char *out, int capacity) {
 struct proc_bsdinfo b={0};
 if(proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&b,sizeof(b))!=sizeof(b)||b.pbi_uid!=getuid()||b.pbi_start_tvsec!=start)return 0;
 int bytes=proc_pidinfo(pid,PROC_PIDLISTFDS,0,NULL,0);
 if(bytes<=0||bytes>1024*1024)return 0;
 struct proc_fdinfo *fds=calloc(1,bytes);
 if(!fds)return 0;
 int count=proc_pidinfo(pid,PROC_PIDLISTFDS,0,fds,bytes)/(int)sizeof(*fds),found=0;
 for(int i=0;i<count;i++) {
  if(fds[i].proc_fdtype!=PROX_FDTYPE_VNODE)continue;
  struct vnode_fdinfowithpath info={0};
  if(proc_pidfdinfo(pid,fds[i].proc_fd,PROC_PIDFDVNODEPATHINFO,&info,sizeof(info))!=sizeof(info))continue;
  const char *path=info.pvip.vip_path;size_t n=strlen(directory);
  if(strncmp(path,directory,n)||path[n]!='/')continue;
  const char *name=path+n+1;size_t len=strlen(name);
  if(strchr(name,'/')||!strstr(name,"_Player_")||strstr(name,"_CrashHandler_")||len<4||strcmp(name+len-4,".log"))continue;
  if(found && strcmp(out,path)){found=0;break;}
  if(strlen(path)>=(size_t)capacity){found=0;break;}
  strlcpy(out,path,capacity);found=1;
 }
 free(fds);
 if(proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&b,sizeof(b))!=sizeof(b)||b.pbi_start_tvsec!=start||b.pbi_uid!=getuid())return 0;
 return found;
}
