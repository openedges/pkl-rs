// Stage57 Java 0.32.0 authority. Independent string-only and export-only evaluator sessions.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Map;
import org.pkl.core.DataSize;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;

public final class DataSizeSplitOracle {
  static int publicCalls, successes, errors, dataSizes, repeats;
  static void bytes(Path folder,String name,byte[] raw) throws Exception { Files.write(folder.resolve(name+".bin"),raw); }
  static void utf8(Path folder,String name,String value) throws Exception {
    bytes(folder,name,value.getBytes(StandardCharsets.UTF_8));
    try(var stream=new DataOutputStream(Files.newOutputStream(folder.resolve(name+".code-units.bin")))) {
      for(int i=0;i<value.length();i++) stream.writeChar(value.charAt(i));
    }
  }
  static String status(Throwable error) {
    if(error instanceof PklBugException) return "oracle-error\n";
    if(error instanceof PklException) return "error\n";
    return "api-exception\n";
  }
  static String message(Throwable error) { return error.getMessage()==null ? error.toString() : error.getMessage(); }
  static Evaluator evaluator() {
    return EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
      .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build();
  }
  static void stringCall(Evaluator evaluator,ModuleSource source,Path folder,String id,String expression,int index) throws Exception {
    String call="string-"+index+"-"+id;
    System.out.println("call-enter="+call);publicCalls++;
    String value="",error="",errorClass="",result="success\n";
    try { value=evaluator.evaluateExpressionString(source,expression);successes++; }
    catch(Throwable thrown) { result=status(thrown);error=message(thrown);errorClass=thrown.getClass().getName();errors++; }
    bytes(folder,"result.status",result.getBytes(StandardCharsets.UTF_8));
    utf8(folder,"result.stdout",value);utf8(folder,"result.stderr",error);utf8(folder,"result.error-class",errorClass);
    System.out.println("call-exit="+call+" status="+result.strip());
  }
  static final class ExportCapture {
    Object value;
    String result="success\n";
  }
  static ExportCapture exportCall(Evaluator evaluator,ModuleSource source,Path folder,String id,String expression,int index,int ordinal) throws Exception {
    String call="export-"+index+"-"+id+"-"+ordinal;
    System.out.println("call-enter="+call);publicCalls++;
    var captured=new ExportCapture();String error="",errorClass="";
    try { captured.value=evaluator.evaluateExpression(source,expression);successes++; }
    catch(Throwable thrown) { captured.result=status(thrown);error=message(thrown);errorClass=thrown.getClass().getName();errors++; }
    String prefix="call-"+ordinal;
    bytes(folder,prefix+".status",captured.result.getBytes(StandardCharsets.UTF_8));
    utf8(folder,prefix+".error",error);utf8(folder,prefix+".error-class",errorClass);
    utf8(folder,prefix+".java-type",captured.value==null?"":captured.value.getClass().getName());
    System.out.println("call-exit="+call+" status="+captured.result.strip());
    String payloadStatus="not-applicable\n",payloadError="",text="",bits="",unit="",hash="";
    if(captured.result.equals("success\n")) {
      System.out.println("payload-enter="+call);
      try {
        text=String.valueOf(captured.value);
        if(captured.value instanceof DataSize size) {
          bits=String.format(Locale.ROOT,"%016x",Double.doubleToRawLongBits(size.getValue()));
          unit=size.getUnit().getSymbol();hash=Integer.toString(size.hashCode());
        }
        payloadStatus="success\n";
      } catch(Throwable thrown) {payloadStatus=status(thrown);payloadError=message(thrown);}
      System.out.println("payload-exit="+call+" status="+payloadStatus.strip());
    }
    bytes(folder,prefix+".payload-status",payloadStatus.getBytes(StandardCharsets.UTF_8));
    utf8(folder,prefix+".payload-error",payloadError);utf8(folder,prefix+".text",text);
    utf8(folder,prefix+".raw-bits-hex",bits);utf8(folder,prefix+".unit",unit);utf8(folder,prefix+".hash",hash);
    return captured;
  }
  static void exportRow(Evaluator evaluator,ModuleSource source,Path folder,String id,String expression,int index) throws Exception {
    var first=exportCall(evaluator,source,folder,id,expression,index,0);
    boolean repeat=first.value instanceof DataSize && first.result.equals("success\n");
    bytes(folder,"repeat-invoked",Boolean.toString(repeat).getBytes(StandardCharsets.UTF_8));
    if(!repeat) return;
    dataSizes++;repeats++;
    var second=exportCall(evaluator,source,folder,id,expression,index,1);
    String comparisonStatus="not-applicable\n",error="",reference="",equals="",hashEqual="";
    if(second.result.equals("success\n")) {
      String call="export-"+index+"-"+id+"-comparison";System.out.println("comparison-enter="+call);
      try {
        reference=Boolean.toString(first.value==second.value);
        equals=Boolean.toString(first.value.equals(second.value));
        hashEqual=Boolean.toString(first.value.hashCode()==second.value.hashCode());
        comparisonStatus="success\n";
      } catch(Throwable thrown) {comparisonStatus=status(thrown);error=message(thrown);}
      System.out.println("comparison-exit="+call+" status="+comparisonStatus.strip());
    }
    bytes(folder,"comparison.status",comparisonStatus.getBytes(StandardCharsets.UTF_8));
    utf8(folder,"comparison.error",error);utf8(folder,"comparison.reference",reference);
    utf8(folder,"comparison.equals",equals);utf8(folder,"comparison.hash-equal",hashEqual);
  }
  public static void main(String[] args) throws Exception {
    if(args.length!=4 || !(args[0].equals("string")||args[0].equals("export")))
      throw new IllegalArgumentException("MODE OUTPUT INPUT URI");
    String mode=args[0];Path out=Path.of(args[1]);Path input=Path.of(args[2]);
    var source=ModuleSource.create(URI.create(args[3]),Files.readString(input.resolve("module.pkl"),StandardCharsets.UTF_8));
    var lines=Files.readAllLines(input.resolve("queries.tsv"),StandardCharsets.UTF_8);
    System.out.println("session-open mode="+mode+" rows="+lines.size());
    try(var evaluator=evaluator()) {
      for(int index=0;index<lines.size();index++) {
        var columns=lines.get(index).split("\t",-1);
        if(columns.length!=2 || !columns[0].matches("[a-z0-9-]+")) throw new IllegalArgumentException("bad row "+index);
        Path folder=out.resolve(String.format(Locale.ROOT,"%03d-%s",index,columns[0]));Files.createDirectory(folder);
        utf8(folder,"expression",columns[1]);
        if(mode.equals("string")) stringCall(evaluator,source,folder,columns[0],columns[1],index);
        else exportRow(evaluator,source,folder,columns[0],columns[1],index);
      }
    }
    System.out.println("session-closed mode="+mode+" rows="+lines.size()+" public-calls="+publicCalls+
      " success="+successes+" errors="+errors+" datasize-rows="+dataSizes+" repeated-export-calls="+repeats);
  }
}
