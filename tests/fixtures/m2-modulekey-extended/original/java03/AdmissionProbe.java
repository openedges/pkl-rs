import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.regex.Pattern;
import org.pkl.core.*;
import org.pkl.core.module.*;

public final class AdmissionProbe {
 static Path work,out;static String policyMode;static boolean cacheMode=true,customMode=true;static String sourceState="good";static int keyInstances,keyUriGets,fragmentGets,originalGets;
 static int events,resolves,imports,secure,denied,factoryCalls,factoryHits,keyResolves,sourceLoads,cacheQueries,readResources,resolveResources;
 static final Map<URI,Plan> plans=new LinkedHashMap<>();
 record Plan(URI requested,URI original,URI resolved,String source,String cache,boolean fragments){}
 static final Map<String,String> texts=new LinkedHashMap<>();
 static final Map<String,ModuleSource> supplied=new LinkedHashMap<>();
 static void bytes(String name,byte[]value)throws IOException{Files.write(out.resolve(name),value);}
 static void text(String name,String value)throws IOException{bytes(name,value.getBytes(StandardCharsets.UTF_8));}
 static void units(String name,String value)throws IOException{try(var stream=new DataOutputStream(Files.newOutputStream(out.resolve(name)))){for(int i=0;i<value.length();i++)stream.writeChar(value.charAt(i));}}
 static void event(String value){try{int index=events++;text("event-"+index+".bin",value);units("event-"+index+".code-units.bin",value);System.out.println(value);}catch(IOException e){throw new UncheckedIOException(e);}}
 static String counters(){return "resolve="+resolves+"\nimport="+imports+"\nsecurePath="+secure+"\ndenied="+denied+"\nfactory="+factoryCalls+"\nfactoryHit="+factoryHits+"\nkeyResolve="+keyResolves+"\nsourceLoad="+sourceLoads+"\nisCached="+cacheQueries+"\npolicyRead="+readResources+"\nresolveResource="+resolveResources+"\nkeyGetUri="+keyUriGets+"\nfragmentPaths="+fragmentGets+"\nresolvedGetOriginal="+originalGets+"\nevent="+events+"\n";}
 static org.pkl.core.SecurityManager policy(){
  var all=List.of(Pattern.compile(".*"));var modules=policyMode.equals("deny-json")?List.of(Pattern.compile("(?!pkl:json$).*")):policyMode.equals("deny-repl")?List.of(Pattern.compile("(?!repl:.*).*")):policyMode.equals("deny-supplied")?List.of(Pattern.compile("(?!memo:/supplied.pkl$).*")):all;
  java.util.function.Function<URI,Integer> trust=uri->{if(policyMode.equals("trust-ref")&&uri.toString().equals("pkl:ref"))return 100;if(policyMode.equals("trust-base")&&uri.toString().equals("pkl:base"))return 100;if(policyMode.equals("trust-json")&&uri.toString().equals("pkl:json"))return 100;return SecurityManagers.defaultTrustLevels.apply(uri);};
  return SecurityManagers.standard(modules,all,trust,null);
 }
 static void denial(String kind,String context,SecurityManagerException error){
  int index=denied++;event("actual-"+kind+"-denied "+context+" denial="+index);
  try{String value=error.getMessage();text("denial-"+index+".kind.bin",kind);text("denial-"+index+".context.bin",context);text("denial-"+index+".message-null.bin",Boolean.toString(value==null));text("denial-"+index+".message.bin",value==null?"":value);units("denial-"+index+".message.code-units.bin",value==null?"":value);text("denial-"+index+".class.bin",error.getClass().getName());}catch(IOException e){throw new UncheckedIOException(e);}
 }
 static org.pkl.core.SecurityManager observedPolicy(){return new org.pkl.core.SecurityManager(){
  public void checkResolveModule(URI uri)throws SecurityManagerException{resolves++;event("policy-resolve-module "+uri);try{policy().checkResolveModule(uri);}catch(SecurityManagerException e){denial("module",uri.toString(),e);throw e;}}
  public void checkImportModule(URI from,URI to)throws SecurityManagerException{imports++;event("policy-import "+from+" -> "+to);try{policy().checkImportModule(from,to);}catch(SecurityManagerException e){denial("import",from+" -> "+to,e);throw e;}}
  public void checkReadResource(URI uri)throws SecurityManagerException{readResources++;event("policy-read-resource "+uri);policy().checkReadResource(uri);}
  public void checkResolveResource(URI uri)throws SecurityManagerException{resolveResources++;event("policy-resolve-resource "+uri);policy().checkResolveResource(uri);}
  public Path resolveSecurePath(URI uri,boolean resource)throws SecurityManagerException,IOException{secure++;event("secure-path "+uri+" resource="+resource);return policy().resolveSecurePath(uri,resource);}
 };}
 static final class PlannedKey implements ModuleKey {
  final Plan plan;final boolean cached;final int id;
  PlannedKey(Plan plan){this.plan=plan;this.cached=plan.cache.equals("dynamic")?cacheMode:plan.cache.equals("true");this.id=keyInstances++;event("key-created id="+id+" requested="+plan.requested+" selected="+plan.original+" cached-snapshot="+cached);}
  public URI getUri(){keyUriGets++;event("key-get-uri id="+id+" "+plan.original);return plan.original;}
  public boolean hasFragmentPaths(){fragmentGets++;event("key-fragment-paths id="+id+" "+plan.fragments);return plan.fragments;}
  public boolean hasHierarchicalUris(){event("key-hierarchical id="+id+" true");return true;}
  public boolean isGlobbable(){return false;}
  public boolean isLocal(){return true;}
  public boolean isCached(){cacheQueries++;event("key-is-cached id="+id+" "+plan.original+" "+cached);return cached;}
  public ResolvedModuleKey resolve(org.pkl.core.SecurityManager security)throws SecurityManagerException{
   keyResolves++;event("key-resolve id="+id+" "+plan.original);security.checkResolveModule(plan.original);event("key-resolved id="+id+" "+plan.original+" -> "+plan.resolved);
   return new ResolvedModuleKey(){
    public ModuleKey getOriginal(){originalGets++;event("resolved-get-original id="+id+" "+plan.original);return PlannedKey.this;}
    public URI getUri(){event("resolved-get-uri id="+id+" "+plan.resolved);return plan.resolved;}
    public String loadSource()throws IOException{
     int index=sourceLoads++;String source=plan.source.equals("$state")?"state-"+sourceState+".pkl":plan.source;
     event("load-source id="+id+" "+plan.original+" -> "+plan.resolved+" load="+index+" state="+sourceState);
     text("load-"+index+".original-uri.bin",plan.original.toString());text("load-"+index+".resolved-uri.bin",plan.resolved.toString());
     if(plan.source.equals("$state")&&sourceState.equals("io")){text("load-"+index+".failure.bin","stage49-source-io");throw new IOException("stage49-source-io");}
     String value=texts.get(source);if(value==null)throw new IllegalStateException("undeclared source "+source);
     text("load-"+index+".source.bin",value);units("load-"+index+".source.code-units.bin",value);return value;
    }
   };
  }
 }
 static void capture(Evaluator evaluator,String sourceId,int index,String expression)throws IOException{
  String prefix="outer-"+index;text(prefix+".source.bin",sourceId);text(prefix+".expression.bin",expression);text(prefix+".counters-before.bin",counters());
  try{ModuleSource source=supplied.get(sourceId);if(source==null)source=ModuleSource.path(work.resolve(sourceId));String value=evaluator.evaluateExpressionString(source,expression);text(prefix+".status.bin","success\n");text(prefix+".stdout.bin",value);units(prefix+".stdout.code-units.bin",value);bytes(prefix+".stderr.bin",new byte[0]);bytes(prefix+".stderr.code-units.bin",new byte[0]);bytes(prefix+".error-class.bin",new byte[0]);text(prefix+".message-null.bin","false");event(prefix+" success");}
  catch(RuntimeException error){String value=error.getMessage();text(prefix+".status.bin","error\n");bytes(prefix+".stdout.bin",new byte[0]);bytes(prefix+".stdout.code-units.bin",new byte[0]);text(prefix+".stderr.bin",value==null?"":value);units(prefix+".stderr.code-units.bin",value==null?"":value);text(prefix+".error-class.bin",error.getClass().getName());text(prefix+".message-null.bin",Boolean.toString(value==null));var trace=new StringWriter();error.printStackTrace(new PrintWriter(trace));text(prefix+".java-stack.bin",trace.toString());event(prefix+" error "+error.getClass().getName());}
  text(prefix+".counters-after.bin",counters());
 }
 public static void main(String[]args)throws Exception{
  work=Path.of(args[0]);Path output=Path.of(args[1]);String startPolicy=args[2];String entry=args[3];
  // Preload authored Host source outside factory.create (SPI forbids URI-related I/O inside create).
  for(String name:Files.readAllLines(work.resolve("source-inputs.txt"))){if(!name.isEmpty())texts.put(name,Files.readString(work.resolve(name)));}
  for(String line:Files.readAllLines(work.resolve("module-plan.tsv"))){if(line.isEmpty())continue;String[]f=line.split("\t",-1);URI request=URI.create(f[0]);plans.put(request,new Plan(request,URI.create(f[1]),URI.create(f[2]),f[3],f[4],Boolean.parseBoolean(f[5])));}
  for(String line:Files.readAllLines(work.resolve("source-profiles.tsv"))){if(line.isEmpty())continue;String[]f=line.split("\t",-1);supplied.put(f[0],f[1].equals("uri")?ModuleSource.uri(f[2]):f[1].equals("repl")?ModuleSource.text(texts.get(f[3])):ModuleSource.create(URI.create(f[2]),texts.get(f[3])));}
  for(int session=0;session<2;session++){
   out=output.resolve("session-"+session);Files.createDirectories(out);events=resolves=imports=secure=denied=factoryCalls=factoryHits=keyResolves=sourceLoads=cacheQueries=readResources=resolveResources=0;policyMode=startPolicy;cacheMode=true;customMode=true;sourceState="good";keyInstances=keyUriGets=fragmentGets=originalGets=0;
   var builder=EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).setPowerAssertionsEnabled(true).setSecurityManager(observedPolicy());
   var originals=new ArrayList<ModuleKeyFactory>(builder.getModuleKeyFactories());var wrapped=new ArrayList<ModuleKeyFactory>();
   wrapped.add(new ModuleKeyFactory(){public Optional<ModuleKey>create(URI uri)throws IOException{factoryCalls++;event("factory-custom "+uri);if(uri.getScheme().equals("throwing"))throw new IOException("stage48-factory-create");Plan p=customMode?plans.get(uri):null;if(p==null)return Optional.empty();factoryHits++;event("factory-custom-hit "+uri);return Optional.of(new PlannedKey(p));}});
   for(int i=0;i<originals.size();i++){final int number=i;final ModuleKeyFactory delegate=originals.get(i);wrapped.add(new ModuleKeyFactory(){public Optional<ModuleKey>create(URI uri)throws java.net.URISyntaxException,org.pkl.core.externalreader.ExternalReaderProcessException,IOException{factoryCalls++;event("factory-"+number+" "+uri);var key=delegate.create(uri);if(key.isPresent()){factoryHits++;event("factory-hit-"+number+" "+uri+" key="+key.get().getClass().getName()+" stdlib="+ModuleKeys.isStdLibModule(key.get()));}return key;}public void close(){delegate.close();}});}
   builder.setModuleKeyFactories(wrapped);
   try(var evaluator=builder.build()){
    int index=0;for(String operation:Files.readAllLines(work.resolve("operations.tsv"))){
     if(operation.equals("GC")){System.gc();event("system-gc-request");continue;}
     if(operation.startsWith("POLICY\t")){policyMode=operation.substring(7);event("policy-profile "+policyMode);continue;}
     if(operation.startsWith("SOURCE\t")){sourceState=operation.substring(7);event("source-profile "+sourceState);continue;}
     if(operation.startsWith("FACTORY\t")){customMode=operation.substring(8).equals("custom");event("factory-profile "+customMode);continue;}
     if(operation.startsWith("CACHE\t")){cacheMode=operation.substring(6).equals("on");event("cache-profile "+cacheMode);continue;}
     String[]fields=operation.split("\t",2);capture(evaluator,fields[0],index++,fields[1]);
    }
   }
   text("counters.bin",counters());
  }
 }
}
