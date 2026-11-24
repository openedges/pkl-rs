// SPDX-License-Identifier: Apache-2.0
// Test-only offline HTTP, HTTPS (custom CA), and HTTP/CONNECT proxy fixture server for M4.
// Development verification tooling only; no evaluator fallback.
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import com.sun.net.httpserver.HttpsConfigurator;
import com.sun.net.httpserver.HttpsParameters;
import com.sun.net.httpserver.HttpsServer;
import java.io.BufferedInputStream;
import java.io.BufferedWriter;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HexFormat;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import javax.net.ssl.KeyManagerFactory;
import javax.net.ssl.SSLContext;

public final class M4HttpFixtureServer {
  private static final Object LOG_LOCK = new Object();

  private static String json(String value) {
    if (value == null) return "null";
    var result = new StringBuilder("\"");
    for (int i = 0; i < value.length(); i++) {
      char c = value.charAt(i);
      if (c == '"' || c == '\\') result.append('\\').append(c);
      else if (c == '\n') result.append("\\n");
      else if (c == '\r') result.append("\\r");
      else if (c == '\t') result.append("\\t");
      else if (c < 32) result.append(String.format("\\u%04x", (int) c));
      else result.append(c);
    }
    return result.append('"').toString();
  }

  private static String pklString(String value) {
    var result = new StringBuilder("\"");
    for (int i = 0; i < value.length(); i++) {
      char c = value.charAt(i);
      if (c == '"' || c == '\\') result.append('\\').append(c);
      else if (c == '\n') result.append("\\n");
      else if (c == '\r') result.append("\\r");
      else if (c == '\t') result.append("\\t");
      else result.append(c);
    }
    return result.append('"').toString();
  }

  private static String sha256Hex(byte[] body) {
    try {
      return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(body));
    } catch (Exception e) {
      throw new RuntimeException(e);
    }
  }

  private static void logLine(BufferedWriter log, String line) {
    synchronized (LOG_LOCK) {
      try {
        log.write(line);
        log.write('\n');
        log.flush();
      } catch (IOException ignored) {
      }
    }
  }

  private static Path safeResolve(Path root, String relative) throws IOException {
    if (relative.startsWith("/")) relative = relative.substring(1);
    Path candidate = root.resolve(relative).normalize();
    if (!candidate.startsWith(root) || !Files.isRegularFile(candidate)) {
      return null;
    }
    Path real = candidate.toRealPath();
    if (!real.startsWith(root)) {
      return null;
    }
    return real;
  }

  private static byte[] buildEchoHeadersModule(HttpExchange exchange, boolean includeUserAgent) {
    Map<String, List<String>> selected = new TreeMap<>();
    String customUa = null;
    for (var entry : exchange.getRequestHeaders().entrySet()) {
      String lower = entry.getKey().toLowerCase(Locale.ROOT);
      List<String> values = new ArrayList<>(entry.getValue());
      if (lower.startsWith("x-")) {
        selected.put(lower, values);
      } else if (lower.equals("user-agent") && !values.isEmpty()) {
        customUa = values.get(0);
      }
    }
    var sb = new StringBuilder();
    sb.append("path = ").append(pklString(exchange.getRequestURI().getPath())).append('\n');
    sb.append("headers {\n");
    for (var entry : selected.entrySet()) {
      sb.append("  [").append(pklString(entry.getKey())).append("] = List(");
      for (int i = 0; i < entry.getValue().size(); i++) {
        if (i > 0) sb.append(", ");
        sb.append(pklString(entry.getValue().get(i)));
      }
      sb.append(")\n");
    }
    sb.append("}\n");
    if (includeUserAgent) {
      sb.append("userAgent = ").append(customUa == null ? "null" : pklString(customUa)).append('\n');
    } else {
      boolean isDefaultPklUa =
          customUa != null
              && (customUa.startsWith("Pkl/0.32.0 (") || customUa.startsWith("pkl-rs/0.32.0 ("));
      sb.append("hasDefaultPklUserAgent = ").append(isDefaultPklUa ? "true" : "false").append('\n');
    }
    return sb.toString().getBytes(StandardCharsets.UTF_8);
  }

  private static void handleAppRequest(
      HttpExchange exchange,
      String serverKind,
      Path fixturesRoot,
      Path packagesRoot,
      int[] ports,
      BufferedWriter log) {
    String method = exchange.getRequestMethod();
    String path = exchange.getRequestURI().getPath();
    int status = 404;
    String location = null;
    byte[] body = new byte[0];
    try {
      if (!method.equals("GET")) {
        status = 405;
      } else if (path.equals("/echo-headers") || path.equals("/redirect-cross-path/end")) {
        body = buildEchoHeadersModule(exchange, false);
        status = 200;
      } else if (path.equals("/echo-ua")) {
        body = buildEchoHeadersModule(exchange, true);
        status = 200;
      } else if (path.equals("/redirect-cross-path/start")) {
        status = 307;
        location = "/redirect-cross-path/end";
        exchange.getResponseHeaders().add("Location", location);
      } else if (path.startsWith("/status/")) {
        String codeStr = path.substring("/status/".length());
        try {
          int parsed = Integer.parseInt(codeStr);
          if (parsed >= 200 && parsed <= 599) status = parsed;
        } catch (NumberFormatException ignored) {
          status = 400;
        }
      } else if (path.startsWith("/redirect/")) {
        // /redirect/<code>/<target...> -> absolute-path Location: /<target...>
        String rest = path.substring("/redirect/".length());
        int slash = rest.indexOf('/');
        if (slash > 0) {
          status = Integer.parseInt(rest.substring(0, slash));
          location = rest.substring(slash);
          exchange.getResponseHeaders().add("Location", location);
        }
      } else if (path.startsWith("/redirect-rel/")) {
        // /redirect-rel/<code>/<rel_target...> -> relative Location without leading /
        String rest = path.substring("/redirect-rel/".length());
        int slash = rest.indexOf('/');
        if (slash > 0) {
          status = Integer.parseInt(rest.substring(0, slash));
          location = rest.substring(slash + 1);
          exchange.getResponseHeaders().add("Location", location);
        }
      } else if (path.startsWith("/redirect-chain/")) {
        // /redirect-chain/<remaining>/<target...>
        String rest = path.substring("/redirect-chain/".length());
        int slash = rest.indexOf('/');
        if (slash > 0) {
          int remaining = Integer.parseInt(rest.substring(0, slash));
          String target = rest.substring(slash);
          status = 307;
          location = remaining <= 1 ? target : "/redirect-chain/" + (remaining - 1) + target;
          exchange.getResponseHeaders().add("Location", location);
        }
      } else if (path.equals("/redirect-loop/a")) {
        status = 301;
        location = "/redirect-loop/b";
        exchange.getResponseHeaders().add("Location", location);
      } else if (path.equals("/redirect-loop/b")) {
        status = 301;
        location = "/redirect-loop/a";
        exchange.getResponseHeaders().add("Location", location);
      } else if (path.equals("/redirect-no-location")) {
        status = 302;
      } else if (path.equals("/redirect-invalid-location")) {
        status = 301;
        location = "http://not a valid url/";
        exchange.getResponseHeaders().add("Location", location);
      } else if (path.startsWith("/redirect-to-https/")) {
        status = 308;
        location = "https://localhost:" + ports[1] + "/" + path.substring("/redirect-to-https/".length());
        exchange.getResponseHeaders().add("Location", location);
      } else if (path.startsWith("/redirect-to-http/")) {
        status = 308;
        location = "http://localhost:" + ports[0] + "/" + path.substring("/redirect-to-http/".length());
        exchange.getResponseHeaders().add("Location", location);
      } else if (path.startsWith("/slow/")) {
        // /slow/<ms>/<target_file>
        String rest = path.substring("/slow/".length());
        int slash = rest.indexOf('/');
        long delayMs = slash > 0 ? Long.parseLong(rest.substring(0, slash)) : 5000L;
        String fileRel = slash > 0 ? rest.substring(slash + 1) : "modules/hello.pkl";
        try {
          Thread.sleep(Math.min(delayMs, 30000L));
        } catch (InterruptedException ie) {
          Thread.currentThread().interrupt();
        }
        Path local = safeResolve(fixturesRoot, fileRel);
        if (local != null) {
          body = Files.readAllBytes(local);
          status = 200;
        }
      } else if (path.startsWith("/HTTP301/") || path.startsWith("/HTTP307/")) {
        status = path.startsWith("/HTTP301/") ? 301 : 307;
        location = path.substring(8);
        exchange.getResponseHeaders().add("Location", location);
      } else if (path.startsWith("/corrupt-zip/")) {
        body = new byte[] {'P', 'K', 3, 4, 0, 1, 2, 3, 4, 5};
        status = 200;
      } else if (path.startsWith("/corrupt-meta/")) {
        body = "{not-valid-json".getBytes(StandardCharsets.UTF_8);
        status = 200;
      } else {
        // 1. Try fixturesRoot directly (e.g. /modules/..., /resources/..., /orig/..., /rewritten/...)
        Path local = safeResolve(fixturesRoot, path);
        if (local == null && packagesRoot != null) {
          // 2. Try packagesRoot directly (e.g. /pkg.pkl-lang.org/.../*.zip or /releases/.../*.zip or /<pkg>@<ver>/<pkg>@<ver>.zip)
          if (path.endsWith(".zip")) {
            local = safeResolve(packagesRoot, path);
          } else {
            // Package metadata URI: /<prefix>/<leaf> -> <packagesRoot>/<prefix>/<leaf>/<leaf>.json
            String trimmed = path.startsWith("/") ? path.substring(1) : path;
            int lastSlash = trimmed.lastIndexOf('/');
            String leaf = lastSlash >= 0 ? trimmed.substring(lastSlash + 1) : trimmed;
            if (!leaf.isEmpty()) {
              local = safeResolve(packagesRoot, trimmed + "/" + leaf + ".json");
            }
          }
        }
        if (local != null) {
          body = Files.readAllBytes(local);
          status = 200;
        }
      }
      exchange.sendResponseHeaders(status, status == 200 || status == 405 ? body.length : -1);
      if (status == 200 && body.length > 0) {
        exchange.getResponseBody().write(body);
      }
      logLine(
          log,
          "{\"server\":"
              + json(serverKind)
              + ",\"method\":"
              + json(method)
              + ",\"uri\":"
              + json(exchange.getRequestURI().toString())
              + ",\"path\":"
              + json(path)
              + ",\"status\":"
              + status
              + ",\"body_bytes\":"
              + body.length
              + ",\"body_sha256\":"
              + json(sha256Hex(body))
              + ",\"location\":"
              + json(location)
              + "}");
    } catch (Exception error) {
      try {
        exchange.sendResponseHeaders(500, -1);
      } catch (IOException ignored) {
      }
    } finally {
      exchange.close();
    }
  }

  private static String readHeaderBlock(InputStream in) throws IOException {
    var buf = new ByteArrayOutputStream();
    int state = 0;
    int b;
    while ((b = in.read()) != -1) {
      buf.write(b);
      if (buf.size() > 65536) throw new IOException("Header block too large");
      if (state == 0 && b == '\r') state = 1;
      else if (state == 1 && b == '\n') state = 2;
      else if (state == 2 && b == '\r') state = 3;
      else if (state == 3 && b == '\n') break;
      else state = (b == '\r') ? 1 : 0;
    }
    return buf.toString(StandardCharsets.ISO_8859_1);
  }

  private static void pipe(InputStream src, OutputStream dst) {
    byte[] buffer = new byte[8192];
    try {
      int n;
      while ((n = src.read(buffer)) != -1) {
        dst.write(buffer, 0, n);
        dst.flush();
      }
    } catch (IOException ignored) {
    }
  }

  private static void handleProxyClient(
      Socket client,
      Path fixturesRoot,
      int[] ports,
      ExecutorService relayPool,
      BufferedWriter log) {
    try (client) {
      client.setSoTimeout(15000);
      var in = new BufferedInputStream(client.getInputStream());
      var out = client.getOutputStream();
      String block = readHeaderBlock(in);
      if (block.isEmpty()) return;
      String[] lines = block.split("\r\n");
      String[] requestLine = lines[0].split(" ");
      if (requestLine.length < 3) return;
      String method = requestLine[0];
      String target = requestLine[1];

      if (method.equals("CONNECT")) {
        // Only permit loopback targets (127.0.0.1 / localhost) for strict offline safety.
        int colon = target.lastIndexOf(':');
        String host = colon > 0 ? target.substring(0, colon) : target;
        int port = colon > 0 ? Integer.parseInt(target.substring(colon + 1)) : 443;
        boolean allowed = host.equalsIgnoreCase("localhost") || host.equals("127.0.0.1");
        logLine(
            log,
            "{\"server\":\"proxy\",\"method\":\"CONNECT\",\"target\":"
                + json(target)
                + ",\"allowed\":"
                + (allowed ? "true" : "false")
                + "}");
        if (!allowed) {
          out.write("HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
              .getBytes(StandardCharsets.ISO_8859_1));
          out.flush();
          return;
        }
        try (Socket upstream = new Socket(InetAddress.getLoopbackAddress(), port)) {
          upstream.setSoTimeout(15000);
          out.write("HTTP/1.1 200 Connection Established\r\n\r\n"
              .getBytes(StandardCharsets.ISO_8859_1));
          out.flush();
          var upIn = upstream.getInputStream();
          var upOut = upstream.getOutputStream();
          var f1 = relayPool.submit(() -> {
            pipe(in, upOut);
            try { upstream.shutdownOutput(); } catch (IOException ignored) {}
          });
          pipe(upIn, out);
          try { client.shutdownOutput(); } catch (IOException ignored) {}
          try { f1.get(); } catch (Exception ignored) {}
        }
        return;
      }

      // Plain HTTP forward proxy request: GET http://<host>[:port]/<path> HTTP/1.1
      URI uri = URI.create(target);
      String host = uri.getHost() == null ? "" : uri.getHost().toLowerCase(Locale.ROOT);
      String path = uri.getRawPath() == null ? "/" : uri.getRawPath();
      if (host.equals("not.a.valid.host") || host.equals("proxy-only.test")) {
        // Serve directly from fixturesRoot/proxy/<path> without touching DNS.
        Path local = safeResolve(fixturesRoot.resolve("proxy"), path);
        int status = local != null ? 200 : 404;
        byte[] body = local != null ? Files.readAllBytes(local) : new byte[0];
        logLine(
            log,
            "{\"server\":\"proxy\",\"method\":"
                + json(method)
                + ",\"target\":"
                + json(target)
                + ",\"host\":"
                + json(host)
                + ",\"status\":"
                + status
                + ",\"body_bytes\":"
                + body.length
                + "}");
        String header =
            "HTTP/1.1 "
                + status
                + (status == 200 ? " OK" : " Not Found")
                + "\r\nContent-Length: "
                + body.length
                + "\r\nConnection: close\r\n\r\n";
        out.write(header.getBytes(StandardCharsets.ISO_8859_1));
        out.write(body);
        out.flush();
        return;
      }

      if (host.equals("localhost") || host.equals("127.0.0.1")) {
        int port = uri.getPort() > 0 ? uri.getPort() : ports[0];
        logLine(
            log,
            "{\"server\":\"proxy\",\"method\":"
                + json(method)
                + ",\"target\":"
                + json(target)
                + ",\"host\":"
                + json(host)
                + ",\"forward_port\":"
                + port
                + "}");
        StringBuilder forwarded = new StringBuilder();
        forwarded.append(method).append(' ').append(path);
        if (uri.getRawQuery() != null) forwarded.append('?').append(uri.getRawQuery());
        forwarded.append(" HTTP/1.1\r\n");
        for (int i = 1; i < lines.length; i++) {
          if (lines[i].isEmpty()) continue;
          String low = lines[i].toLowerCase(Locale.ROOT);
          if (low.startsWith("proxy-connection:") || low.startsWith("connection:")) continue;
          forwarded.append(lines[i]).append("\r\n");
        }
        forwarded.append("Connection: close\r\n\r\n");
        try (Socket upstream = new Socket(InetAddress.getLoopbackAddress(), port)) {
          upstream.setSoTimeout(15000);
          upstream.getOutputStream().write(forwarded.toString().getBytes(StandardCharsets.ISO_8859_1));
          upstream.getOutputStream().flush();
          pipe(upstream.getInputStream(), out);
        }
        return;
      }

      logLine(
          log,
          "{\"server\":\"proxy\",\"method\":"
              + json(method)
              + ",\"target\":"
              + json(target)
              + ",\"status\":403}");
      out.write("HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
          .getBytes(StandardCharsets.ISO_8859_1));
      out.flush();
    } catch (Exception ignored) {
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 5) {
      throw new IllegalArgumentException(
          "Usage: M4HttpFixtureServer <fixtures_root> <packages_root> <keystore_p12> <readiness_json> <request_log_jsonl>");
    }
    Path fixturesRoot = Path.of(args[0]).toRealPath();
    Path packagesRoot = args[1].equals("-") ? null : Path.of(args[1]).toRealPath();
    Path keystorePath = Path.of(args[2]);
    Path readinessPath = Path.of(args[3]);
    Path logPath = Path.of(args[4]);

    var keyStore = KeyStore.getInstance("PKCS12");
    try (var ksIn = Files.newInputStream(keystorePath)) {
      keyStore.load(ksIn, "password".toCharArray());
    }
    var kmf = KeyManagerFactory.getInstance("SunX509");
    kmf.init(keyStore, "password".toCharArray());
    var sslContext = SSLContext.getInstance("TLS");
    sslContext.init(kmf.getKeyManagers(), null, null);
    var engine = sslContext.createSSLEngine();

    InetAddress loopback = InetAddress.getLoopbackAddress();
    var httpServer = HttpServer.create(new InetSocketAddress(loopback, 0), 0);
    var httpsServer = HttpsServer.create(new InetSocketAddress(loopback, 0), 0);
    httpsServer.setHttpsConfigurator(new HttpsConfigurator(sslContext) {
      @Override
      public void configure(HttpsParameters parameters) {
        parameters.setNeedClientAuth(false);
        parameters.setCipherSuites(engine.getEnabledCipherSuites());
        parameters.setProtocols(engine.getEnabledProtocols());
        parameters.setSSLParameters(sslContext.getSupportedSSLParameters());
      }
    });
    var proxySocket = new ServerSocket(0, 50, loopback);

    int[] ports = new int[] {
      httpServer.getAddress().getPort(),
      httpsServer.getAddress().getPort(),
      proxySocket.getLocalPort()
    };

    ExecutorService httpPool = Executors.newFixedThreadPool(4);
    ExecutorService httpsPool = Executors.newFixedThreadPool(4);
    ExecutorService proxyPool = Executors.newCachedThreadPool();
    httpServer.setExecutor(httpPool);
    httpsServer.setExecutor(httpsPool);

    try (BufferedWriter log = Files.newBufferedWriter(
        logPath,
        StandardCharsets.UTF_8,
        StandardOpenOption.CREATE_NEW,
        StandardOpenOption.WRITE)) {
      httpServer.createContext(
          "/", exchange -> handleAppRequest(exchange, "http", fixturesRoot, packagesRoot, ports, log));
      httpsServer.createContext(
          "/", exchange -> handleAppRequest(exchange, "https", fixturesRoot, packagesRoot, ports, log));

      Thread proxyAcceptThread = new Thread(() -> {
        while (!proxySocket.isClosed()) {
          try {
            Socket client = proxySocket.accept();
            proxyPool.submit(() -> handleProxyClient(client, fixturesRoot, ports, proxyPool, log));
          } catch (IOException e) {
            break;
          }
        }
      }, "m4-proxy-accept");
      proxyAcceptThread.setDaemon(true);

      try {
        httpServer.start();
        httpsServer.start();
        proxyAcceptThread.start();
        String readyJson =
            "{\"http_port\":"
                + ports[0]
                + ",\"https_port\":"
                + ports[1]
                + ",\"proxy_port\":"
                + ports[2]
                + "}\n";
        Path readyTmp = readinessPath.resolveSibling(readinessPath.getFileName().toString() + ".tmp");
        Files.writeString(readyTmp, readyJson, StandardCharsets.UTF_8, StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
        Files.move(readyTmp, readinessPath);
        // Block until parent closes stdin for clean deterministic shutdown.
        while (System.in.read() != -1) {
          // ignore
        }
      } finally {
        try { proxySocket.close(); } catch (IOException ignored) {}
        httpServer.stop(0);
        httpsServer.stop(0);
        httpPool.shutdownNow();
        httpsPool.shutdownNow();
        proxyPool.shutdownNow();
      }
    }
  }
}
