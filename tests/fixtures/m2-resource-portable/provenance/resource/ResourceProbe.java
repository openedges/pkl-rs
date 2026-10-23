import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import org.pkl.core.*;
import org.pkl.core.resource.ResourceReader;
public final class ResourceProbe {
 static Path work,out;static int reads,events;static String format;static boolean counted;
 static int resolves,imports,resolveResources,securePaths,denied,policyReads;static String mode;static Map<URI,Integer>attempts;
 static String counters(){return "resolve="+resolves+"\nimport="+imports+"\nread="+reads+"\npolicyRead="+policyReads+"\nresolveResource="+resolveResources+"\nsecurePath="+securePaths+"\ndenied="+denied+"\nevent="+events+"\n";}
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
  work=Path.of(args[0]);Path root=Path.of(args[1]);mode=args[2];
  for(int session=0;session<2;session++){
   out=root.resolve("session-"+session);Files.createDirectories(out);reads=events=resolves=imports=resolveResources=securePaths=denied=policyReads=0;attempts=new HashMap<>();
   var allowed=mode.equals("policy")?List.of(java.util.regex.Pattern.compile("res:(?!/denied)"),java.util.regex.Pattern.compile("unknown:"),java.util.regex.Pattern.compile("prop:"),java.util.regex.Pattern.compile("env:")):List.of(java.util.regex.Pattern.compile(".*"));
   var policy=SecurityManagers.standard(List.of(java.util.regex.Pattern.compile(".*")),allowed,SecurityManagers.defaultTrustLevels,null);
   org.pkl.core.SecurityManager observed=new org.pkl.core.SecurityManager(){
    public void checkResolveModule(URI uri)throws SecurityManagerException{resolves++;logEvent("resolve "+uri);policy.checkResolveModule(uri);}
    public void checkImportModule(URI from,URI to)throws SecurityManagerException{imports++;logEvent("import "+from+" -> "+to);policy.checkImportModule(from,to);}
    public void checkReadResource(URI uri)throws SecurityManagerException{policyReads++;logEvent("policy-read "+uri);try{policy.checkReadResource(uri);}catch(SecurityManagerException e){denied++;try{text("denial-"+denied+".message.bin",e.getMessage());units("denial-"+denied+".message.code-units.bin",e.getMessage());}catch(IOException x){throw new UncheckedIOException(x);}logEvent("actual-policy-denied "+uri);throw e;}}
    public void checkResolveResource(URI uri)throws SecurityManagerException{resolveResources++;logEvent("policy-resolve-resource "+uri);policy.checkResolveResource(uri);}
    public Path resolveSecurePath(URI uri,boolean resource)throws SecurityManagerException,IOException{securePaths++;logEvent("secure-path "+uri+" resource="+resource);return policy.resolveSecurePath(uri,resource);}
   };
   var builder=EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).setPowerAssertionsEnabled(true).setSecurityManager(observed);
   builder.addResourceReader(new ResourceReader(){
    public String getUriScheme(){return "res";}public boolean hasHierarchicalUris(){return true;}public boolean isGlobbable(){return false;}
    public Optional<Object> read(URI uri)throws IOException{
     reads++;int n=attempts.merge(uri,1,Integer::sum);logEvent("reader "+uri+" attempt="+n+" total="+reads);
     String path=uri.getPath();
     if(path!=null&&path.endsWith("/missing"))return Optional.empty();
     if(path!=null&&path.endsWith("/retry")&&n==1)throw new IOException("reader-first");
     if(path!=null&&path.endsWith("/io-null"))throw new IOException((String)null);
     if(path!=null&&path.endsWith("/io-empty"))throw new IOException("");
     if(path!=null&&path.endsWith("/io-units"))throw new IOException(" \ud800 \r\n\treader ");
     if(path!=null&&path.startsWith("/vector"))return Optional.of(new org.pkl.core.resource.Resource(URI.create("res:/returned"+path),Files.readAllBytes(work.resolve("payloads"+path+".bin"))));
     if(path!=null&&path.endsWith("/resource"))return Optional.of(new org.pkl.core.resource.Resource(URI.create("res:/different-returned-uri"),new byte[]{65,13,10,0,66}));
     return Optional.of("payload-"+reads+" "+uri);
    }
    public void close(){logEvent("reader-close");}
   });
   try(var evaluator=builder.build()){
    int n=0;for(String operation:Files.readAllLines(work.resolve("operations.tsv"))){
     if(operation.equals("GC")){System.gc();event("explicit-System.gc-request");continue;}
     String[]fields=operation.split("\t",2);text("outer-"+n+".source.bin",fields[0]);capture(evaluator,work.resolve(fields[0]),"outer-"+(n++),fields[1]);
    }
   }
   text("counters.bin",counters());
  }
 }
}
