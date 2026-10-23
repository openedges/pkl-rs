import java.net.URI;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.*;
import org.pkl.core.SecurityManagers;
import org.pkl.core.module.ModuleKeyFactories;
import org.pkl.core.runtime.*;
import org.pkl.core.util.IoUtils;
public final class IdentityProbe {
 static String b64(String text) { return Base64.getEncoder().encodeToString(text.getBytes(StandardCharsets.UTF_8)); }
 public static void main(String[] args) throws Exception {
  var resolver=new ModuleResolver(List.of(ModuleKeyFactories.file));
  for(var line:Files.readAllLines(Path.of(args[0]))) {
   var parts=line.split("\t",-1);var base=resolver.resolve(URI.create(parts[1]));
   var absolute=parts[2].isEmpty()?base.getUri():IoUtils.resolve(SecurityManagers.defaultManager,base,IoUtils.toUri(new String(Base64.getDecoder().decode(parts[2]),StandardCharsets.UTF_8)));
   var key=resolver.resolve(absolute);
   try {
    var resolved=key.resolve(SecurityManagers.defaultManager);var source=VmUtils.loadSource(resolved);
    System.out.println(parts[0]+"\t"+b64(absolute.toString())+"\t"+b64(key.getUri().toString())+"\t"+b64(resolved.getUri().toString())+"\t"+b64(source.getURI().toString())+"\t"+b64(IoUtils.inferModuleName(key))+"\t"+HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(source.getCharacters().toString().getBytes(StandardCharsets.UTF_8))));
   } catch(Exception e) {System.out.println(parts[0]+"\t"+b64(absolute.toString())+"\t"+b64(key.getUri().toString())+"\tERROR\t"+b64(e.getClass().getName())+"\t"+b64(String.valueOf(e.getMessage())));}
  }
 }
}
