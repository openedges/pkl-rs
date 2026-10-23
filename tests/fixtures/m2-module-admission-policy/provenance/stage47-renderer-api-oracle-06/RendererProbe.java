import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import org.pkl.core.*;
import org.pkl.core.resource.ResourceReader;
public final class RendererProbe {
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
   String deniedUri=mode.equals("deny-json")?"pkl:json":"pkl:(base|ref|test)";
   var all=List.of(java.util.regex.Pattern.compile(".*"));
   var excluding=List.of(java.util.regex.Pattern.compile(mode.equals("deny-json")?"(?!pkl:json$).*":"(?!pkl:(?:base|ref|test)$).*"));
   var policy=SecurityManagers.standard(mode.startsWith("deny-")?excluding:all,mode.equals("deny-resource")?excluding:all,SecurityManagers.defaultTrustLevels,null);
   org.pkl.core.SecurityManager observed=new org.pkl.core.SecurityManager(){
    public void checkResolveModule(URI uri)throws SecurityManagerException{resolves++;logEvent("policy-resolve-module "+uri);try{policy.checkResolveModule(uri);}catch(SecurityManagerException e){denied++;logEvent("actual-module-denied "+uri);throw e;}}
    public void checkImportModule(URI from,URI to)throws SecurityManagerException{imports++;logEvent("policy-import "+from+" -> "+to);policy.checkImportModule(from,to);}
    public void checkReadResource(URI uri)throws SecurityManagerException{policyReads++;logEvent("policy-read-resource "+uri);try{policy.checkReadResource(uri);}catch(SecurityManagerException e){denied++;logEvent("actual-resource-denied "+uri);throw e;}}
    public void checkResolveResource(URI uri)throws SecurityManagerException{resolveResources++;logEvent("policy-resolve-resource "+uri);policy.checkResolveResource(uri);}
    public Path resolveSecurePath(URI uri,boolean resource)throws SecurityManagerException,IOException{securePaths++;logEvent("secure-path "+uri+" resource="+resource);return policy.resolveSecurePath(uri,resource);}
   };
   var plans=new LinkedHashMap<String,String[]>(); for(String line:Files.readAllLines(work.resolve("reader.tsv"))){if(line.isEmpty())continue;String[]row=line.split("\t",-1);plans.put(row[0],row);}
   var builder=EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).setPowerAssertionsEnabled(true).setSecurityManager(observed);
   builder.addResourceReader(new ResourceReader(){
    public String getUriScheme(){return "res";}public boolean hasHierarchicalUris(){return true;}public boolean isGlobbable(){return false;}
    public Optional<Object>read(URI uri)throws IOException{
     String key=uri.toString();int attempt=attempts.merge(uri,1,Integer::sum);int number=reads++;logEvent("reader "+key+" attempt="+attempt+" total="+reads);String[]plan=plans.get(key);text("read-"+number+".uri.bin",key);text("read-"+number+".attempt.bin",Integer.toString(attempt));
     if(plan==null){text("read-"+number+".kind.bin","io");text("read-"+number+".message.bin","unconfigured-reader");throw new IOException("unconfigured-reader");}
     if(plan[1].equals("io-first")&&attempt==1){text("read-"+number+".kind.bin","io");text("read-"+number+".message.bin","renderer-reader-first");throw new IOException("renderer-reader-first");}
     if(plan[1].equals("missing")){text("read-"+number+".kind.bin","missing");return Optional.empty();}
     byte[]payload=Files.readAllBytes(work.resolve(plan[2]));if(payload.length%2!=0)throw new IOException("odd-captured-utf16");StringBuilder value=new StringBuilder();try(var stream=new DataInputStream(new ByteArrayInputStream(payload))){while(stream.available()>0)value.append(stream.readChar());}String result=value.toString();text("read-"+number+".kind.bin","text");units("read-"+number+".value.code-units.bin",result);text("read-"+number+".value.bin",result);return Optional.of(result);
    }
   });
   try(var evaluator=builder.build()){
    int n=0;for(String operation:Files.readAllLines(work.resolve("operations.tsv"))){if(operation.equals("GC")){System.gc();logEvent("system-gc-request");continue;}String[]fields=operation.split("\t",2);text("outer-"+n+".source.bin",fields[0]);capture(evaluator,work.resolve(fields[0]),"outer-"+(n++),fields[1]);}
   }
   text("counters.bin",counters());text("denied-uri.bin",deniedUri);
  }
 }
}
