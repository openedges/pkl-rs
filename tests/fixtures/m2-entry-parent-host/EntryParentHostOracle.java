import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.regex.Pattern;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;
import org.pkl.core.module.ModuleKey;
import org.pkl.core.module.ModuleKeyFactory;
import org.pkl.core.module.ResolvedModuleKey;

public final class EntryParentHostOracle {
  static final URI MAIN = URI.create("stage68:/entry-parent-host/main.pkl");
  static final URI ALIASES = URI.create("stage68:/entry-parent-host/aliases.pkl");
  static int calls, successes, errors;
  static void bytes(Path p, String n, byte[] b) throws Exception { Files.write(p.resolve(n + ".bin"), b); }
  static void string(Path p, String n, String value) throws Exception {
    bytes(p, n, value.getBytes(StandardCharsets.UTF_8));
    try (var stream = new DataOutputStream(Files.newOutputStream(p.resolve(n + ".code-units.bin")))) {
      for (int i = 0; i < value.length(); i++) stream.writeChar(value.charAt(i));
    }
  }
  static String status(Throwable t) {
    if (t instanceof PklBugException) return "oracle-error\n";
    if (t instanceof PklException) return "error\n";
    return "api-exception\n";
  }
  static String message(Throwable t) { return t.getMessage() == null ? t.toString() : t.getMessage(); }
  static final class MemoryKey implements ModuleKey {
    final URI uri;
    final String source;
    final List<String> events;
    MemoryKey(URI u, String s, List<String> e) { uri=u; source=s; events=e; }
    public URI getUri() { return uri; }
    public boolean hasHierarchicalUris() { return true; }
    public boolean isGlobbable() { return false; }
    public ResolvedModuleKey resolve(org.pkl.core.SecurityManager securityManager) {
      events.add("resolve\t" + uri);
      return new ResolvedModuleKey() {
        public ModuleKey getOriginal() { return MemoryKey.this; }
        public URI getUri() { return uri; }
        public String loadSource() { events.add("load\t" + uri); return source; }
      };
    }
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("OUTPUT INPUT");
    Path output=Path.of(args[0]), input=Path.of(args[1]);
    String mainText=Files.readString(input.resolve("main.pkl"), StandardCharsets.UTF_8);
    String aliasText=Files.readString(input.resolve("aliases.pkl"), StandardCharsets.UTF_8);
    var main=ModuleSource.create(MAIN, mainText);
    var rows=Files.readAllLines(input.resolve("queries.tsv"), StandardCharsets.UTF_8);
    var events=new ArrayList<String>();
    ModuleKeyFactory factory=uri -> {
      if (!"stage68".equals(uri.getScheme())) return Optional.empty();
      events.add("create\t" + uri);
      if (uri.equals(MAIN)) return Optional.of(new MemoryKey(uri, mainText, events));
      if (uri.equals(ALIASES)) return Optional.of(new MemoryKey(uri, aliasText, events));
      throw new IllegalArgumentException("undeclared module request: " + uri);
    };
    var builder=EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
      .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of())
      .setAllowedModules(List.of(Pattern.compile(".*")));
    var factories=new ArrayList<ModuleKeyFactory>(builder.getModuleKeyFactories());
    factories.add(0, factory);
    builder.setModuleKeyFactories(factories);
    System.out.println("session-open mode=string rows=" + rows.size());
    try (var evaluator=builder.build()) {
      for (int i=0; i<rows.size(); i++) {
        var columns=rows.get(i).split("\t",-1);
        if (columns.length!=3 || !columns[0].equals("string") || !columns[1].matches("[a-z0-9-]+"))
          throw new IllegalArgumentException("bad row " + i);
        var mode=columns[0]; var id=columns[1]; var expression=columns[2];
        Path folder=output.resolve(String.format(Locale.ROOT,"%03d-%s",i,id));
        Files.createDirectory(folder);
        string(folder,"mode",mode); string(folder,"expression",expression);
        String call="string-"+i+"-"+id;
        System.out.println("call-enter="+call); calls++;
        int eventStart=events.size();
        String stdout="",stderr="",errorClass="",exceptionToString="",causeClass="",causeMessage="",causeToString="",javaType="",result="success\n";
        try { stdout=evaluator.evaluateExpressionString(main,expression); successes++; }
        catch (Throwable t) {
          result=status(t); stderr=message(t); errorClass=t.getClass().getName(); exceptionToString=t.toString();
          if (t.getCause()!=null) { causeClass=t.getCause().getClass().getName(); causeMessage=message(t.getCause()); causeToString=t.getCause().toString(); }
          errors++;
        }
        bytes(folder,"result.status",result.getBytes(StandardCharsets.UTF_8));
        string(folder,"result.stdout",stdout); string(folder,"result.stderr",stderr);
        string(folder,"result.error-class",errorClass); string(folder,"result.exception-to-string",exceptionToString);
        string(folder,"result.cause-class",causeClass); string(folder,"result.cause-message",causeMessage);
        string(folder,"result.cause-to-string",causeToString); string(folder,"result.java-type",javaType);
        string(folder,"module-events",String.join("\n",events.subList(eventStart,events.size()))+(events.size()>eventStart?"\n":""));
        System.out.println("call-exit="+call+" status="+result.strip());
      }
    }
    System.out.println("session-closed mode=string rows="+rows.size()+" public-calls="+calls+" success="+successes+" errors="+errors);
  }
}
