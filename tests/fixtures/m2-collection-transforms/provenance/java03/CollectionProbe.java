// Development-only original Java 0.32.0 API collector; no production evaluator.
import java.io.DataOutputStream;
import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.regex.Pattern;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.Logger;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;
import org.pkl.core.StackFrame;
import org.pkl.core.evaluatorSettings.TraceMode;
import org.pkl.core.resource.ResourceReader;

public final class CollectionProbe {
  static Path out; static int events,reads,logs,callIndex;
  static final HexFormat HEX=HexFormat.of();
  static void text(Path path,String value) throws IOException {Files.write(path,value.getBytes(StandardCharsets.UTF_8));}
  static void string(String name,String value) throws IOException {
    text(out.resolve(name+".bin"),value);
    try(var stream=new DataOutputStream(Files.newOutputStream(out.resolve(name+".utf16be.bin")))) {for(int i=0;i<value.length();i++)stream.writeChar(value.charAt(i));}
  }
  static String event(String kind) throws IOException {
    String prefix="event-"+(events++);string(prefix+".kind",kind);string(prefix+".call-index",String.valueOf(callIndex));return prefix;
  }
  static final class CaptureLogger implements Logger {
    public void trace(String message,StackFrame frame){record("trace",message,frame);}
    public void warn(String message,StackFrame frame){record("warn",message,frame);}
    void record(String level,String message,StackFrame frame){try{
      logs++;String p=event("logger");string(p+".level",level);string(p+".message",message);string(p+".module-uri",frame.getModuleUri());
      string(p+".member-null",String.valueOf(frame.getMemberName()==null));string(p+".member",frame.getMemberName()==null?"":frame.getMemberName());
      string(p+".position",frame.getStartLine()+":"+frame.getStartColumn()+":"+frame.getEndLine()+":"+frame.getEndColumn());
      string(p+".source-lines",String.valueOf(frame.getSourceLines().size()));
      for(int n=0;n<frame.getSourceLines().size();n++)string(p+".source-"+n,frame.getSourceLines().get(n));
    }catch(IOException e){throw new RuntimeException(e);}}
  }
  record Plan(String kind,String value){}
  static final class Reader implements ResourceReader {
    final Map<String,Plan> plan;final Map<String,Integer> attempts=new HashMap<>();
    Reader(Map<String,Plan> p){plan=p;}
    public String getUriScheme(){return "ct";}
    public boolean hasHierarchicalUris(){return true;}
    public boolean isGlobbable(){return false;}
    public Optional<Object> read(URI uri) throws IOException {
      reads++;String u=uri.toString();int attempt=attempts.merge(u,1,Integer::sum);String p=event("reader-call");string(p+".uri",u);string(p+".attempt",String.valueOf(attempt));
      Plan rule=plan.get(u);
      if(rule==null){string(p+".result","unconfigured");throw new IOException("stage51-unconfigured-reader-uri:"+u);}
      if(rule.kind().equals("io-first") && attempt==1){string(p+".result","io");string(p+".message",rule.value());throw new IOException(rule.value());}
      if(rule.kind().equals("io")){string(p+".result","io");string(p+".message",rule.value());throw new IOException(rule.value());}
      string(p+".result","text");String value=rule.kind().equals("io-first")?"recovered":rule.value();string(p+".value",value);return Optional.of(value);
    }
    public void close(){try{event("reader-close");}catch(IOException e){throw new RuntimeException(e);}}
  }
  public static void main(String[] args) throws Exception {
    if(args.length!=4)throw new IllegalArgumentException("SOURCE OPERATIONS READER_PLAN OUTPUT");
    Path sourcePath=Path.of(args[0]);var operations=Files.readAllLines(Path.of(args[1]));Map<String,Plan> plan=new HashMap<>();
    for(String line:Files.readAllLines(Path.of(args[2]))){var row=line.split("\t",-1);if(row.length!=3)throw new IllegalArgumentException("reader plan columns");if(plan.put(row[0],new Plan(row[1],new String(HEX.parseHex(row[2]),StandardCharsets.UTF_8)))!=null)throw new IllegalArgumentException("duplicate reader");}
    Path root=Path.of(args[3]);Files.createDirectories(root);
    for(int session=0;session<2;session++){
      out=root.resolve("session-"+session);Files.createDirectory(out);events=reads=logs=callIndex=0;
      Evaluator evaluator=EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false).setPowerAssertionsEnabled(false)
          .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).setTraceMode(TraceMode.COMPACT)
          .setLogger(new CaptureLogger()).setAllowedResources(List.of(Pattern.compile("ct:.*"))).setResourceReaders(List.of(new Reader(plan))).setTimeout(Duration.ofSeconds(8)).build();
      ModuleSource source=ModuleSource.path(sourcePath);event("evaluator-created");
      for(String line:operations){var fields=line.split("\t",-1);if(fields.length!=2)throw new IllegalArgumentException("operation columns");String kind=fields[0];String expression=new String(HEX.parseHex(fields[1]),StandardCharsets.UTF_8);
        if(kind.equals("gc")){event("gc-request");System.gc();continue;}
        if(kind.equals("close")){event("close-call");evaluator.close();event("close-return");continue;}
        if(!kind.equals("evaluate"))throw new IllegalArgumentException("operation kind");
        String p="call-"+callIndex;string(p+".expression",expression);string(p+".counts-before",reads+"\t"+logs+"\t"+events);String result="",error="",errorClass="",status="success\n";boolean nullMessage=false;
        event("api-call");
        try{result=evaluator.evaluateExpressionString(source,expression);}
        catch(PklException e){status=e instanceof PklBugException?"oracle-error\n":"error\n";error=e.getMessage();nullMessage=error==null;if(nullMessage)error="";errorClass=e.getClass().getName();}
        catch(Throwable e){status="api-error\n";nullMessage=e.getMessage()==null;error=nullMessage?"":e.getMessage();errorClass=e.getClass().getName();string(p+".throwable-to-string",e.toString());}
        string(p+".stdout",result);string(p+".stderr",error);string(p+".status",status);string(p+".error-class",errorClass);string(p+".message-null",String.valueOf(nullMessage));
        event("api-result");string(p+".counts-after",reads+"\t"+logs+"\t"+events);callIndex++;
      }
      event("final-close-call");evaluator.close();event("final-close-return");string("session-counts",callIndex+"\t"+reads+"\t"+logs+"\t"+events);
    }
  }
}
