using System;using System.IO;using System.Diagnostics;using System.Threading;using System.Runtime.InteropServices;
public sealed class SuspendedCapture:IDisposable {
[DllImport("ntdll.dll")] static extern int NtSuspendProcess(IntPtr h);
[DllImport("ntdll.dll")] static extern int NtResumeProcess(IntPtr h);
readonly FileSystemWatcher watcher;readonly string exe,root,log;readonly int pid;int captured;
public SuspendedCapture(string dir,string exePath,string fixture,string output,int process){exe=exePath;root=fixture;log=output;pid=process;watcher=new FileSystemWatcher(dir,"*.tmp");watcher.Created+=Capture;watcher.EnableRaisingEvents=true;}
void Capture(object sender,FileSystemEventArgs e){if(Interlocked.CompareExchange(ref captured,1,0)!=0)return;using(var editor=Process.GetProcessById(pid)){bool suspended=false;try{var ready=Stopwatch.StartNew(); bool readable=false; while(ready.ElapsedMilliseconds<1000){try{using(var f=new FileStream(e.FullPath,FileMode.Open,FileAccess.Read,FileShare.ReadWrite|FileShare.Delete)){readable=true;}break;}catch(IOException){Thread.Yield();}} File.AppendAllText(log,"readable="+readable+" wait_ms="+ready.Elapsed.TotalMilliseconds+Environment.NewLine); int status=NtSuspendProcess(editor.Handle);suspended=status==0;File.AppendAllText(log,"source="+e.FullPath+" suspend="+status+Environment.NewLine);if(!suspended)return;var p=new Process();p.StartInfo=new ProcessStartInfo(exe,"inspect-discovery \""+root+"\""){UseShellExecute=false,CreateNoWindow=true,RedirectStandardOutput=true,RedirectStandardError=true};p.Start();var stdout=p.StandardOutput.ReadToEnd();var stderr=p.StandardError.ReadToEnd();p.WaitForExit();File.AppendAllText(log,"exit="+p.ExitCode+Environment.NewLine+stdout+stderr);}catch(Exception error){File.AppendAllText(log,error.ToString());}finally{if(suspended)File.AppendAllText(log,"resume="+NtResumeProcess(editor.Handle)+Environment.NewLine);}}}
public void Dispose(){watcher.Dispose();}
}



