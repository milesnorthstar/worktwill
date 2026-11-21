using System;using System.IO;using System.Diagnostics;using System.Threading;
public sealed class SourceCapture:IDisposable {
readonly FileSystemWatcher watcher;readonly string exe,root,log;int captured;
public SourceCapture(string dir,string exePath,string fixture,string output){exe=exePath;root=fixture;log=output;watcher=new FileSystemWatcher(dir,"*.tmp");watcher.Created+=Capture;watcher.EnableRaisingEvents=true;}
void Capture(object sender,FileSystemEventArgs e){if(Interlocked.CompareExchange(ref captured,1,0)!=0)return;try{using(var file=new FileStream(e.FullPath,FileMode.Open,FileAccess.Read,FileShare.ReadWrite)){File.AppendAllText(log,"held="+e.FullPath+Environment.NewLine);var p=new Process();p.StartInfo=new ProcessStartInfo(exe,"scan-once \""+root+"\""){UseShellExecute=false,CreateNoWindow=true,RedirectStandardOutput=true,RedirectStandardError=true};p.Start();var stdout=p.StandardOutput.ReadToEnd();var stderr=p.StandardError.ReadToEnd();p.WaitForExit();File.AppendAllText(log,"exit="+p.ExitCode+Environment.NewLine+stdout+stderr);}}catch(Exception error){File.AppendAllText(log,error.ToString());}finally{File.AppendAllText(log,Environment.NewLine+"released="+DateTime.UtcNow.ToString("o")+Environment.NewLine);}}
public void Dispose(){watcher.Dispose();}
}
