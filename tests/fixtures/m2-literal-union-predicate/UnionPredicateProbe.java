package org.pkl.core.ast.type;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.io.*;
import java.util.*;
import com.oracle.truffle.api.source.Source;
import com.oracle.truffle.api.source.SourceSection;
public final class UnionPredicateProbe {
  record Recipe(boolean optimized, int defaultIndex, List<String> literals) {}
  static final Map<String,Recipe> types = new LinkedHashMap<>();
  static final String[][] pairs = {
    {"optimized-reordered","s_ab","s_ba"},
    {"optimized-duplicates","s_ab","s_dup"},
    {"optimized-duplicates-reverse","s_dup","s_ab"},
    {"optimized-no-vs-default","s_ab","s_d0"},
    {"optimized-different-defaults","s_d0","s_d1"},
    {"optimized-vs-general","s_ab","g_ab"},
    {"general-vs-optimized","g_ab","s_ab"},
    {"general-reordered","g_ab","g_ba"},
    {"general-duplicates","g_ab","g_dup"},
    {"general-no-vs-default","g_ab","g_d0"},
    {"general-different-defaults","g_d0","g_d1"},
    {"optimized-different-set","s_ab","s_ac"},
    {"general-different-child","g_ab","g_ac"},
    {"optimized-unpaired-reordered","s_hl","s_lh"},
    {"unpaired-optimized-vs-general","s_hl","g_hl"},
    {"unpaired-general-vs-optimized","g_hl","s_hl"}
  };
  static void recipe(String name,boolean optimized,int def,String...values) { types.put(name,new Recipe(optimized,def,List.of(values))); }
  static TypeNode node(String name,SourceSection section) {
    var r=types.get(name);
    if(r.optimized()) return new TypeNode.UnionOfStringLiteralsTypeNode(section,r.defaultIndex(),new LinkedHashSet<>(r.literals()));
    return new TypeNode.UnionTypeNode(section,r.defaultIndex(),r.literals().stream().map(s->new TypeNode.StringLiteralTypeNode(section,s)).toArray(TypeNode[]::new));
  }
  static void text(Path out,String name,String value)throws IOException {
    Files.write(out.resolve(name+".bin"),value.getBytes(StandardCharsets.UTF_8));
    try(var d=new DataOutputStream(Files.newOutputStream(out.resolve(name+".code-units.bin")))) { for(int i=0;i<value.length();i++)d.writeChar(value.charAt(i)); }
  }
  public static void main(String[] args)throws Exception {
    recipe("s_ab",true,-1,"a","b");recipe("s_ba",true,-1,"b","a");recipe("s_dup",true,-1,"a","b","a");
    recipe("s_d0",true,0,"a","b");recipe("s_d1",true,1,"a","b");recipe("s_ac",true,-1,"a","c");
    recipe("g_ab",false,-1,"a","b");recipe("g_ba",false,-1,"b","a");recipe("g_dup",false,-1,"a","b","a");
    recipe("g_d0",false,0,"a","b");recipe("g_d1",false,1,"a","b");recipe("g_ac",false,-1,"a","c");
    recipe("s_hl",true,-1,"\ud83d","\udc00");recipe("s_lh",true,-1,"\udc00","\ud83d");recipe("g_hl",false,-1,"\ud83d","\udc00");
    var source=Source.newBuilder("pkl","\"a\" | \"b\"","union-predicate-internal").build();
    var section=source.createSection(0,source.getLength());
    var out=Path.of(args[0]);Files.createDirectories(out);
    for(var pair:pairs) {
      var id=pair[0];text(out,id+".left-recipe",types.get(pair[1]).toString());text(out,id+".right-recipe",types.get(pair[2]).toString());
      try {
        var left=node(pair[1],section);var right=node(pair[2],section);
        text(out,id+".left-class",left.getClass().getName());text(out,id+".right-class",right.getClass().getName());
        var answer=left.isEquivalentTo(right);text(out,id+".status","success\n");text(out,id+".stdout",Boolean.toString(answer));text(out,id+".stderr","");text(out,id+".error-class","");
        System.out.println(id+"\t"+answer);
      } catch(RuntimeException e) {
        text(out,id+".status","error\n");text(out,id+".stdout","");text(out,id+".stderr",String.valueOf(e.getMessage()));text(out,id+".error-class",e.getClass().getName());
        var stack=new StringWriter();e.printStackTrace(new PrintWriter(stack));text(out,id+".java-stack",stack.toString());System.out.println(id+"\tERROR\t"+e.getClass().getName());
      }
    }
  }
}
