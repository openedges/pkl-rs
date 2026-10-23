// Development-only pinned Java evaluator observation; never production code.
import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.time.Duration;
import java.util.*;
import java.util.regex.Pattern;
import org.pkl.core.*;
import org.pkl.core.module.*;
import org.pkl.core.resource.ResourceReader;

public final class GuestCatchProbe {
  static Path work,out;
  static String mode;
  static Evaluator evaluator;
  static int resolves,sources,reads,logs,events,callbacks;
  static boolean inCallback;
  static final String UNITS = new String(new char[]{'\u0085','\u00a0','L','\ud83d','|','\udc00','|','\ud83d','\ude00','|','\u0000','R','\u0085','\u00a0'});
  static final String WHITE = "\t \u001c\r\nalpha\nbeta\rgamma\r\n\u2003";
  static final String BLANK = "\u001c\u001d\u001e\u001f\u2003\t \n ";
  static void bytes(String name,byte[] value)throws IOException{Files.write(out.resolve(name),value);}
  static void text(String name,String value)throws IOException{bytes(name,value.getBytes(StandardCharsets.UTF_8));}
  static void units(String name,String value)throws IOException{try(var d=new DataOutputStream(Files.newOutputStream(out.resolve(name)))){for(int i=0;i<value.length();i++)d.writeChar(value.charAt(i));}}
  static void event(String value)throws IOException{text("event-"+(events++)+".bin",value);System.out.println(value);}
  static void capture(String prefix,String expression)throws IOException{
    text(prefix+".expression.bin",expression);
    try{
      String value=evaluator.evaluateExpressionString(ModuleSource.path(work.resolve("main.pkl")),expression);
      text(prefix+".status.bin","success\n");text(prefix+".stdout.bin",value);units(prefix+".stdout.code-units.bin",value);
      bytes(prefix+".stderr.bin",new byte[0]);bytes(prefix+".stderr.code-units.bin",new byte[0]);bytes(prefix+".error-class.bin",new byte[0]);text(prefix+".message-null.bin","false");event(prefix+" success");
    }catch(RuntimeException failure){
      String message=failure.getMessage();text(prefix+".status.bin","error\n");bytes(prefix+".stdout.bin",new byte[0]);bytes(prefix+".stdout.code-units.bin",new byte[0]);
      text(prefix+".message-null.bin",Boolean.toString(message==null));text(prefix+".stderr.bin",message==null?"":message);units(prefix+".stderr.code-units.bin",message==null?"":message);
      text(prefix+".error-class.bin",failure.getClass().getName());var stack=new StringWriter();failure.printStackTrace(new PrintWriter(stack));text(prefix+".java-stack.bin",stack.toString());event(prefix+" error "+failure.getClass().getName());
    }
  }
  static void callback()throws IOException{
    if(inCallback||callbacks!=0)return;
    inCallback=true;event("callback-start");
    try{for(String expression:Files.readAllLines(work.resolve("callbacks.txt")))capture("inner-"+(callbacks++),expression);}
    finally{inCallback=false;event("callback-end");}
  }
  static void delay(String operation)throws IOException{
    event(operation+" sleep-start");
    try{Thread.sleep(4000);}catch(InterruptedException failure){event(operation+" interrupted");throw new IOException(operation+"-interrupted",failure);}
    event(operation+" sleep-end");
  }
  static final class Key implements ModuleKey{
    final URI uri;Key(URI uri){this.uri=uri;}
    public URI getUri(){return uri;}
    public boolean hasHierarchicalUris(){return true;}
    public boolean isGlobbable(){return false;}
    public ResolvedModuleKey resolve(org.pkl.core.SecurityManager security)throws IOException,SecurityManagerException{
      security.checkResolveModule(uri);int attempt=++resolves;event("resolve "+uri+" attempt="+attempt);
      if(mode.equals("resolve-retry")&&attempt==1)throw new IOException("resolve-first");
      return new ResolvedModuleKey(){
        public ModuleKey getOriginal(){return Key.this;}
        public URI getUri(){return uri;}
        public String loadSource()throws IOException{
          int attempt=++sources;event("source-start "+uri+" attempt="+attempt);
          if(mode.equals("source-reentry")&&attempt==1)callback();
          if((mode.equals("source-cache")||mode.equals("source-reentry"))&&attempt==1)throw new IOException("source-first");
          if(mode.equals("source-timeout"))delay("source");
          if(mode.equals("source-close")){event("source close");evaluator.close();}
          event("source-end "+uri+" attempt="+attempt);return "value = 9\n";
        }
      };
    }
  }
  static void log(String message,StackFrame frame)throws IOException{
    String p="log-"+(logs++);text(p+".level.bin","trace");text(p+".message.bin",message);units(p+".message.code-units.bin",message);
    text(p+".uri.bin",frame.getModuleUri());units(p+".uri.code-units.bin",frame.getModuleUri());
    text(p+".member-null.bin",Boolean.toString(frame.getMemberName()==null));text(p+".member.bin",frame.getMemberName()==null?"":frame.getMemberName());units(p+".member.code-units.bin",frame.getMemberName()==null?"":frame.getMemberName());
    text(p+".position.bin",frame.getStartLine()+":"+frame.getStartColumn()+":"+frame.getEndLine()+":"+frame.getEndColumn());text(p+".source-count.bin",Integer.toString(frame.getSourceLines().size()));
    for(int i=0;i<frame.getSourceLines().size();i++){text(p+".source-"+i+".bin",frame.getSourceLines().get(i));units(p+".source-"+i+".code-units.bin",frame.getSourceLines().get(i));}
    event("trace "+message);if(mode.equals("logger-reentry"))callback();
  }
  public static void main(String[] args)throws Exception{
    work=Path.of(args[0]);out=Path.of(args[1]);mode=args[2];Files.createDirectories(out);
    var builder=EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).setPowerAssertionsEnabled(true);
    var factories=new ArrayList<ModuleKeyFactory>();factories.add(uri->Optional.ofNullable("catch".equals(uri.getScheme())?new Key(uri):null));factories.addAll(builder.getModuleKeyFactories());builder.setModuleKeyFactories(factories);
    var allowedModules=new ArrayList<Pattern>(SecurityManagers.defaultAllowedModules);allowedModules.add(Pattern.compile("catch:"));builder.setAllowedModules(allowedModules);
    var allowedResources=new ArrayList<Pattern>(SecurityManagers.defaultAllowedResources);allowedResources.add(Pattern.compile("catch:"));builder.setAllowedResources(allowedResources);
    builder.addResourceReader(new ResourceReader(){
      public String getUriScheme(){return "catch";}
      public boolean hasHierarchicalUris(){return true;}
      public boolean isGlobbable(){return false;}
      public Optional<Object> read(URI uri)throws IOException{
        int attempt=++reads;event("read "+uri+" attempt="+attempt);
        if(mode.equals("renderer"))return Optional.of(switch(uri.getPath()){case "/units"->UNITS;case "/white"->WHITE;case "/blank"->BLANK;default->throw new IOException("unknown-payload");});
        if(mode.equals("read-reentry")&&attempt==1)callback();
        if((mode.equals("read-retry")||mode.equals("read-reentry"))&&attempt==1)throw new IOException("read-first");
        if(mode.equals("read-timeout"))delay("read");
        if(mode.equals("read-close")){event("read close");evaluator.close();}
        return Optional.of("9");
      }
    });
    if(mode.endsWith("-timeout"))builder.setTimeout(Duration.ofSeconds(3));
    builder.setLogger(new Logger(){public void trace(String message,StackFrame frame){try{log(message,frame);}catch(IOException failure){throw new UncheckedIOException(failure);}}});
    evaluator=builder.build();
    try{int index=0;for(String expression:Files.readAllLines(work.resolve("expressions.txt")))capture("outer-"+(index++),expression);}
    finally{evaluator.close();}
    text("counters.bin","resolve="+resolves+"\nsource="+sources+"\nread="+reads+"\nlog="+logs+"\ncallback="+callbacks+"\nevent="+events+"\n");
    text("payload-units.bin",UNITS);units("payload-units.code-units.bin",UNITS);text("payload-white.bin",WHITE);units("payload-white.code-units.bin",WHITE);text("payload-blank.bin",BLANK);units("payload-blank.code-units.bin",BLANK);
  }
}
