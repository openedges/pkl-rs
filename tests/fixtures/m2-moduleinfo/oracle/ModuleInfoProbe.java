import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import org.pkl.core.*;
import org.pkl.core.resource.ResourceReader;
public final class ModuleInfoProbe {
 static Path work,out;static int reads,events;static String format;static boolean counted;
 static void bytes(String name,byte[]value)throws IOException{Files.write(out.resolve(name),value);}
 static void text(String name,String value)throws IOException{bytes(name,value.getBytes(StandardCharsets.UTF_8));}
 static void units(String name,String value)throws IOException{try(var d=new DataOutputStream(Files.newOutputStream(out.resolve(name)))){for(int i=0;i<value.length();i++)d.writeChar(value.charAt(i));}}
 static void event(String value)throws IOException{text("event-"+(events++)+".bin",value);System.out.println(value);}
 static void capture(Evaluator evaluator,Path source,String prefix,String expression)throws IOException{
  text(prefix+".expression.bin",expression);text(prefix+".reads-before.bin",Integer.toString(reads));
  try{
   String value=evaluator.evaluateExpressionString(ModuleSource.path(source),expression);
   text(prefix+".status.bin","success\n");text(prefix+".stdout.bin",value);units(prefix+".stdout.code-units.bin",value);bytes(prefix+".stderr.bin",new byte[0]);bytes(prefix+".stderr.code-units.bin",new byte[0]);bytes(prefix+".error-class.bin",new byte[0]);text(prefix+".message-null.bin","false");event(prefix+" success");
  }catch(RuntimeException error){
   String value=error.getMessage();text(prefix+".status.bin","error\n");bytes(prefix+".stdout.bin",new byte[0]);bytes(prefix+".stdout.code-units.bin",new byte[0]);text(prefix+".message-null.bin",Boolean.toString(value==null));text(prefix+".stderr.bin",value==null?"":value);units(prefix+".stderr.code-units.bin",value==null?"":value);text(prefix+".error-class.bin",error.getClass().getName());var stack=new StringWriter();error.printStackTrace(new PrintWriter(stack));text(prefix+".java-stack.bin",stack.toString());event(prefix+" error "+error.getClass().getName());
  }
  text(prefix+".reads-after.bin",Integer.toString(reads));
 }
 public static void main(String[]args)throws Exception {
  work=Path.of(args[0]);Path root=Path.of(args[1]);
  var builder=EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).setPowerAssertionsEnabled(true);
  try(var evaluator=builder.build()) {
   for(int i=2;i<args.length;i++) {
    Path workspace=work.resolve(args[i]);out=root.resolve(args[i]);Files.createDirectories(out);reads=0;events=0;
    int n=0;for(String expression:Files.readAllLines(workspace.resolve("expressions.txt")))capture(evaluator,workspace.resolve("main.pkl"),"outer-"+(n++),expression);
    capture(evaluator,work.resolve("control.pkl"),"followup-0","stable");text("counters.bin","read="+reads+"\nevent="+events+"\n");
   }
  }
 }
}
