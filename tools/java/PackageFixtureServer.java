// SPDX-License-Identifier: Apache-2.0
// Test-only adaptation of Pkl 0.32.0 PackageServer.kt; no evaluator fallback.
import com.sun.net.httpserver.HttpsConfigurator;
import com.sun.net.httpserver.HttpsParameters;
import com.sun.net.httpserver.HttpsServer;
import java.io.BufferedWriter;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.concurrent.Executors;
import javax.net.ssl.KeyManagerFactory;
import javax.net.ssl.SSLContext;

public final class PackageFixtureServer {
  private static String json(String value) {
    var result = new StringBuilder("\"");
    for (int i = 0; i < value.length(); i++) {
      char c = value.charAt(i);
      if (c == '"' || c == '\\') result.append('\\').append(c);
      else if (c < 32) result.append(String.format("\\u%04x", (int) c));
      else result.append(c);
    }
    return result.append('"').toString();
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 4) throw new IllegalArgumentException("packages keystore readiness request-log");
    Path packages = Path.of(args[0]).toRealPath();
    Path readiness = Path.of(args[2]);
    var keyStore = KeyStore.getInstance("PKCS12");
    try (var input = Files.newInputStream(Path.of(args[1]))) {
      keyStore.load(input, "password".toCharArray());
    }
    var manager = KeyManagerFactory.getInstance("SunX509");
    manager.init(keyStore, "password".toCharArray());
    var context = SSLContext.getInstance("SSL");
    context.init(manager.getKeyManagers(), null, null);
    var engine = context.createSSLEngine();
    var server = HttpsServer.create(new InetSocketAddress(0), 0);
    server.setHttpsConfigurator(new HttpsConfigurator(context) {
      @Override public void configure(HttpsParameters parameters) {
        parameters.setNeedClientAuth(false);
        parameters.setCipherSuites(engine.getEnabledCipherSuites());
        parameters.setProtocols(engine.getEnabledProtocols());
        parameters.setSSLParameters(context.getSupportedSSLParameters());
      }
    });
    var executor = Executors.newFixedThreadPool(1);
    server.setExecutor(executor);
    try (BufferedWriter log = Files.newBufferedWriter(Path.of(args[3]), StandardCharsets.UTF_8,
        StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE)) {
      server.createContext("/", exchange -> {
        String method = exchange.getRequestMethod();
        String path = exchange.getRequestURI().getPath();
        int status = 404;
        String location = null;
        byte[] body = new byte[0];
        try {
          if (!method.equals("GET")) status = 405;
          else if (path.startsWith("/HTTP301/") || path.startsWith("/HTTP307/")) {
            status = path.startsWith("/HTTP301/") ? 301 : 307;
            location = path.substring(8);
            exchange.getResponseHeaders().add("Location", location);
          } else {
            String relative = path.endsWith(".zip") ? path.substring(1)
                : path.substring(1) + path + ".json";
            Path local = packages.resolve(relative).normalize();
            // The bounded fixture server never exposes files outside its qualified root.
            if (local.startsWith(packages) && Files.isRegularFile(local)
                && local.toRealPath().startsWith(packages)) {
              body = Files.readAllBytes(local);
              status = 200;
            }
          }
          exchange.sendResponseHeaders(status, status == 200 || status == 405 ? 0 : -1);
          if (status == 200) exchange.getResponseBody().write(body);
          String hash = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(body));
          log.write("{\"method\":" + json(method) + ",\"uri\":"
              + json(exchange.getRequestURI().toString()) + ",\"path\":" + json(path)
              + ",\"status\":" + status + ",\"body_bytes\":" + body.length
              + ",\"body_sha256\":" + json(hash) + ",\"location\":"
              + (location == null ? "null" : json(location)) + "}\n");
          log.flush();
        } catch (Exception error) {
          error.printStackTrace(System.err);
        } finally {
          exchange.close();
        }
      });
      try {
        server.start();
        Files.writeString(readiness, "{\"port\":" + server.getAddress().getPort() + "}\n",
            StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
        // Parent closes stdin to end the owned server without leaving a JVM behind.
        System.in.read();
      } finally {
        server.stop(0);
        executor.shutdownNow();
      }
    }
  }
}
