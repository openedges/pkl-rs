import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import org.pkl.core.*;
import org.pkl.core.resource.ResourceReader;
public final class DenialProbe {
 static Path work,out;static int reads,events;static String format;static boolean counted;
 static int resolves,imports,resolveResources,securePaths,denied;
 static String counters(){return "resolve="+resolves+"\nimport="+imports+"\nread="+reads+"\nresolveResource="+resolveResources+"\nsecurePath="+securePaths+"\ndenied="+denied+"\nevent="+events+"\n";}
 static void bytes(String name,byte[]value)throws IOException{Files.write(out.resolve(name),value);}
 static void text(String name,String value)throws IOException{bytes(name,value.getBytes(StandardCharsets.UTF_8));}
 static void units(String name,String value)throws IOException{try(var d=new DataOutputStream(Files.newOutputStream(out.resolve(name)))){for(int i=0;i<value.length();i++)d.writeChar(value.charAt(i));}}
 static void event(String value)throws IOException{text("event-"+(events++)+".bin",value);System.out.println(value);}
 static void capture(Evaluator evaluator,Path source,String prefix,String expression)throws IOException{
  text(prefix+".expression.bin",expression);text(prefix+".reads-before.bin",Integer.toString(reads));text(prefix+".counters-before.bin",counters());
  try{
   String value=evaluator.evaluateExpressionString(ModuleSource.path(source),expression);
   text(prefix+".status.bin","success\n");text(prefix+".stdout.bin",value);units(prefix+".stdout.code-units.bin",value);bytes(prefix+".stderr.bin",new byte[0]);bytes(prefix+".stderr.code-units.bin",new byte[0]);bytes(prefix+".error-class.bin",new byte[0]);text(prefix+".message-null.bin","false");event(prefix+" success");
  }catch(RuntimeException error){
   String value=error.getMessage();text(prefix+".status.bin","error\n");bytes(prefix+".stdout.bin",new byte[0]);bytes(prefix+".stdout.code-units.bin",new byte[0]);text(prefix+".message-null.bin",Boolean.toString(value==null));text(prefix+".stderr.bin",value==null?"":value);units(prefix+".stderr.code-units.bin",value==null?"":value);text(prefix+".error-class.bin",error.getClass().getName());var stack=new StringWriter();error.printStackTrace(new PrintWriter(stack));text(prefix+".java-stack.bin",stack.toString());event(prefix+" error "+error.getClass().getName());
  }
  text(prefix+".reads-after.bin",Integer.toString(reads));text(prefix+".counters-after.bin",counters());
 }
 static void logEvent(String value){try{event(value);}catch(IOException e){throw new UncheckedIOException(e);}}
 public static void main(String[]args)throws Exception{
  work=Path.of(args[0]);out=Path.of(args[1]);Files.createDirectories(out);
  URI main=work.resolve("main.pkl").toUri(),child=work.resolve("child.pkl").toUri();
  var allowed=List.of(java.util.regex.Pattern.compile(java.util.regex.Pattern.quote(main.toString())+"$"),java.util.regex.Pattern.compile("pkl:"),java.util.regex.Pattern.compile("repl:"));
  var policy=SecurityManagers.standard(allowed,SecurityManagers.defaultAllowedResources,SecurityManagers.defaultTrustLevels,null);
  org.pkl.core.SecurityManager observed=new org.pkl.core.SecurityManager(){
   public void checkResolveModule(URI uri)throws SecurityManagerException{
    resolves++;logEvent("resolve "+uri);
    try{policy.checkResolveModule(uri);}catch(SecurityManagerException error){
     String stem="denial-"+(denied++);
     try{text(stem+".uri.bin",uri.toString());text(stem+".error-class.bin",error.getClass().getName());text(stem+".message.bin",error.getMessage());units(stem+".message.code-units.bin",error.getMessage());}catch(IOException e){throw new UncheckedIOException(e);}
     logEvent("denied "+uri+" "+error.getClass().getName());throw error;
    }
   }
   public void checkImportModule(URI from,URI to)throws SecurityManagerException{imports++;logEvent("import "+from+" -> "+to);policy.checkImportModule(from,to);}
   public void checkReadResource(URI uri)throws SecurityManagerException{reads++;logEvent("read "+uri);policy.checkReadResource(uri);}
   public void checkResolveResource(URI uri)throws SecurityManagerException{resolveResources++;logEvent("resolve-resource "+uri);policy.checkResolveResource(uri);}
   public Path resolveSecurePath(URI uri,boolean resource)throws SecurityManagerException,IOException{securePaths++;logEvent("secure-path "+uri+" resource="+resource);return policy.resolveSecurePath(uri,resource);}
  };
  var builder=EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).setPowerAssertionsEnabled(true).setSecurityManager(observed);
  try(var evaluator=builder.build()){int n=0;for(String expression:Files.readAllLines(work.resolve("expressions.txt")))capture(evaluator,work.resolve("main.pkl"),"outer-"+(n++),expression);}
  text("counters.bin",counters());text("existing-child-uri.bin",child.toString());text("allowed-main-pattern.bin",allowed.get(0).pattern());
 }
}
