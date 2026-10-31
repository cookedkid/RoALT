// A separately supervised cooperative CPU governor. No task ports or process-memory access.
#include <sys/types.h>
#include <sys/wait.h>
#include <sys/sysctl.h>
#include <sys/proc.h>
#include <libproc.h>
#include <mach/mach_time.h>
#include <poll.h>
#include <signal.h>
#include <unistd.h>
#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>

static uint64_t now_ns(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (uint64_t)t.tv_sec*1000000000ULL+t.tv_nsec;}
struct identity{pid_t pid;uint64_t sec,usec;};
static int info(struct identity id,struct proc_bsdinfo *b){
 if(proc_pidinfo(id.pid,PROC_PIDTBSDINFO,0,b,sizeof(*b))!=sizeof(*b))return 0;
 return b->pbi_uid==getuid()&&b->pbi_start_tvsec==id.sec&&b->pbi_start_tvusec==id.usec;
}
static int same(struct identity id){struct proc_bsdinfo b={0};return info(id,&b);}
static int resume(struct identity id){return same(id)?kill(id.pid,SIGCONT):0;}
static int cpu_ns(struct identity id,uint64_t *cpu){
 struct rusage_info_v2 r={0};if(!same(id)||proc_pid_rusage(id.pid,RUSAGE_INFO_V2,(rusage_info_t*)&r))return 0;
 mach_timebase_info_data_t t;mach_timebase_info(&t);
 *cpu=(uint64_t)(((__uint128_t)r.ri_user_time+r.ri_system_time)*t.numer/t.denom);return 1;
}
// This watchdog survives both RoALT and governor death. It resumes only when armed by
// the governor, and checks microsecond process identity before every signal.
static void guard(int fd,struct identity id){
 setsid();close(STDIN_FILENO);close(STDOUT_FILENO);
 int armed=0;uint64_t heartbeat=now_ns();
 for(;;){
  struct pollfd p={fd,POLLIN|POLLHUP,0};int n=poll(&p,1,50);
  if(n<0&&errno==EINTR)continue;
  if(n<0)break;
  if(n>0){char commands[128];ssize_t got=read(fd,commands,sizeof(commands));
   if(got<=0)break;
   heartbeat=now_ns();for(ssize_t i=0;i<got;i++){if(commands[i]=='S')armed=1;else if(commands[i]=='R')armed=0;}
  }
  if(!same(id))break;
  // A hung/stopped governor cannot keep its client suspended. Keep rescuing while
  // the lease is expired in case its last STOP races this timeout.
  if(armed&&now_ns()-heartbeat>300000000ULL)resume(id);
 }
 if(armed){for(int retry=0;retry<20&&resume(id)!=0;retry++)usleep(50000);}
 close(fd);_exit(0);
}
int ro_cpu_governor(int pid,uint64_t expected,int percent){
 if(pid<=1||percent<1||percent>100)return 2;
 struct proc_bsdinfo b={0};if(proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&b,sizeof(b))!=sizeof(b)||b.pbi_uid!=getuid()||b.pbi_start_tvsec!=expected)return 3;
 // Never take ownership of suspension imposed by someone else.
 if(b.pbi_status==SSTOP)return 4;
 struct identity id={pid,b.pbi_start_tvsec,b.pbi_start_tvusec};
 int fds[2];if(pipe(fds))return 5;
 signal(SIGPIPE,SIG_IGN);
 pid_t watcher=fork();if(watcher<0){close(fds[0]);close(fds[1]);return 6;}
 if(!watcher){close(fds[1]);guard(fds[0],id);}
 close(fds[0]);
 uint64_t previous=0;int result=0,stopped=0;
 if(!cpu_ns(id,&previous)){result=3;goto done;}
 // Ready only once the recovery watchdog exists and the identity has been checked.
 if(write(STDOUT_FILENO,"R",1)!=1){result=7;goto done;}
 int cores=1;size_t len=sizeof(cores);sysctlbyname("hw.logicalcpu",&cores,&len,NULL,0);if(cores<1)cores=1;
 long double allowance=(long double)percent*cores/100.0L,balance=0;
 uint64_t before=now_ns(),stopped_at=0;
 for(;;){
  struct pollfd input={STDIN_FILENO,POLLIN|POLLHUP,0};
  int n=poll(&input,1,stopped?10:5);
  if(n<0&&errno==EINTR)continue;
  if(n<0)break;
  if(n>0){char ch;if(read(STDIN_FILENO,&ch,1)<=0)break;}
  if(waitpid(watcher,NULL,WNOHANG)==watcher){result=8;break;}
  uint64_t cpu=0,now=now_ns();if(!cpu_ns(id,&cpu))break;
  balance+=(now-before)*allowance-(cpu>=previous?cpu-previous:0);
  long double burst=allowance*10000000.0L;if(balance>burst)balance=burst;
  previous=cpu;before=now;
  if(stopped){
   if(write(fds[1],"H",1)!=1){result=8;break;}
   if(balance>=0||now-stopped_at>=100000000ULL){
    if(resume(id)){result=9;break;}
    stopped=0;if(write(fds[1],"R",1)!=1){result=8;break;}
   }
  }else if(balance<-1000000.0L){
   if(!info(id,&b)||b.pbi_status==SSTOP){result=4;break;}
   // Arm recovery before STOP; a failure can never leave an unguarded suspension.
   if(write(fds[1],"S",1)!=1){result=8;break;}
   stopped=1;stopped_at=now;
   if(!same(id)||kill(pid,SIGSTOP)){result=9;break;}
  }
 }
 done:
 if(stopped&&resume(id)==0){write(fds[1],"R",1);}
 close(fds[1]);
 // Watchdog gets EOF and makes a final identity-checked rescue before it exits.
 while(waitpid(watcher,NULL,0)<0&&errno==EINTR){}
 return result;
}
#ifdef ROALT_STANDALONE
int main(int argc,char **argv){if(argc!=4)return 2;return ro_cpu_governor(atoi(argv[1]),strtoull(argv[2],NULL,10),atoi(argv[3]));}
#endif
