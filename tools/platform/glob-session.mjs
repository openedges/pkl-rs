// Shared finite Host driver for actual Node and Chromium SourceSession bindings.
// URI operations delegate production shared Rust entry points. No glob evaluator
// or Java SPI callback simulator is implemented here.
import {codeUnits, parseGlobFixture, GlobFixtureProvider} from './glob-fixture.mjs';
const must = (condition, message) => { if (!condition) throw Error(message); };
const id = value => { must(typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value), 'decimal identity required'); return value; };
const text = value => { must(typeof value === 'string', 'actual JavaScript string required'); return value; };
const copy = value => {
  if (ArrayBuffer.isView(value)) return Array.from(value);
  if (Array.isArray(value)) return value.map(copy);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k,v]) => [k,copy(v)]));
  return value;
};
const sourceForValue = value => `name = "${value.replaceAll('\\','\\\\').replaceAll('"','\\"').replaceAll('\n','\\n').replaceAll('\r','\\r')}"\n`;

export class GlobFixtureHost {
  constructor(session, fixture) {
    this.session=session; this.provider=new GlobFixtureProvider(fixture);
    this.resources=new Map(); this.modules=new Map(); this.genericUrls=new Map(); this.effects=[];
  }
  call(method, ...args) {
    must(typeof this.session[method] === 'function', `required production binding absent: ${method}`);
    const before=copy(args);
    const result=this.session[method](...args);
    this.effects.push({phase:this.provider.phase, direction:'completion', method, arguments:before, result:copy(result)});
    return result;
  }
  policy(uri, resource) {
    text(uri);
    const patterns=this.provider.fixture.config.get(resource?'allowedResources':'allowedModules');
    must(typeof patterns === 'string', 'finite allowlist missing');
    for (const pattern of patterns.split(',')) {
      const prefix={'gmain:.*':'gmain:', 'gdata:.*':'gdata:', 'gmod:.*':'gmod:', 'pkl:.*':'pkl:', 'repl:text':'repl:text'}[pattern];
      must(prefix!==undefined,'unconfigured finite allowlist pattern');
      if(uri.startsWith(prefix)) return null;
    }
    const [verb,noun,option]=resource?['read','resource','resources']:['load','module','modules'];
    return {kind:'security-manager-exception',message:`Refusing to ${verb} ${noun} \`${uri}\` because it does not match any entry in the ${noun} allowlist (\`--allowed-${option}\`).`};
  }
  failure(rule, operation) {
    if (!rule) return null;
    // These helper paths throw unchecked Java exceptions. They need their real
    // API/internal classification; no typed ordinary rejection substitutes.
    if (rule.kind==='unchecked' || operation==='fragment-paths'
        || (['globbable','hierarchical'].includes(operation) && rule.kind==='security')
        || (['resolve','resolve-uri','load-source'].includes(operation) && rule.kind==='external-reader')
        || (operation==='load-source' && rule.kind==='security'))
      throw Error(`unrepresented Java unchecked callback: ${operation}/${rule.kind}`);
    const kinds={io:'io', security:'security-manager-exception', 'external-reader':'external-reader-process'};
    if (kinds[rule.kind]) return {kind:kinds[rule.kind],message:rule.value};
    if (rule.kind==='missing' && ['resolve','load-source'].includes(operation)) return {kind:'not-found',message:undefined};
    must(['missing','value','elements'].includes(rule.kind),'unknown fixture failure kind');
    return null;
  }
  script(operation, uri) {
    const rule=this.provider.rule(operation,uri);
    return {rule,failure:this.failure(rule,operation)};
  }
  genericUrl(request, glob=false) {
    const selected=this.genericUrls.get(id(glob?request.readerId:request.keyId));
    if(!selected) return false;
    must((glob?request.readerKind==='module':true)
      && request.keyKind==='host'
      && (glob?request.readerUri:request.originalUri)===selected.uri
      && (glob?request.readerHandle:request.keyHandle)===selected.handle,
      'changed GenericUrl module identity');
    return true;
  }
  moduleReader(request) {
    const reader=this.modules.get(id(request.keyId));
    must(reader && reader.owner===request.keyHandle && request.keyKind==='host','changed selected module identity');
    // HasElement.originalUri is the queried element, not the selected key URI.
    if(request.kind!=='has-element') must(reader.selectedUri===request.originalUri,'changed selected module URI');
    return reader;
  }
  globReader(request) {
    if(request.readerKind==='module') {
      const reader=this.modules.get(id(request.readerId));
      must(reader && request.readerUri===reader.selectedUri && request.readerHandle===reader.owner && request.keyKind==='host','changed glob module identity');
      return reader;
    }
    must(request.readerKind==='resource','unknown glob reader kind');
    const reader=this.resources.get(id(request.readerId));
    must(reader && request.readerScheme===reader.scheme && request.readerHandle===reader.owner,'changed glob resource identity');
    return reader;
  }
  glob(token, request) {
    id(request.id);id(request.readerId);
    if(request.kind==='select-resource') {
      const uri=text(request.uri);must(uri.includes(':'),'selected resource lacks scheme');
      const reader=this.provider.resourceReader(uri.slice(0,uri.indexOf(':')));
      if(!reader) return this.call('rejectGlob',token,'reader-not-found',undefined,0);
      must(!this.resources.has(request.readerId),'reused resource reader identity');
      this.resources.set(request.readerId,reader);
      return this.call('completeGlobReader',token,request.readerId,reader.scheme,reader.owner,0);
    }
    if(request.readerKind==='module' && this.genericUrl(request,true)) {
      if(request.kind==='is-globbable') return this.call('completeGlobBoolean',token,request.kind,false,0);
      if(request.kind==='is-hierarchical') return this.call('completeGlobBoolean',token,request.kind,true,0);
      throw Error('GenericUrl is not globbable; unexpected element operation');
    }
    const reader=this.globReader(request);
    const operation={'is-globbable':'globbable','is-hierarchical':'hierarchical','fragment-paths':'fragment-paths','has-element':'has-element','list-elements':'list-elements'}[request.kind];
    must(operation,'unknown glob request kind');
    const capability=['globbable','hierarchical','fragment-paths'].includes(operation);
    const uri=capability?reader.selectedUri:text(request.uri);
    const {rule,failure}=this.script(operation,uri);
    const denied=failure??(capability?null:this.policy(uri,request.readerKind==='resource'));
    if(denied) return this.call('rejectGlob',token,denied.kind,denied.message,0);
    if(capability) return this.call('completeGlobBoolean',token,request.kind,this.provider.boolean(reader.owner,operation==='fragment-paths'?'fragmentPaths':operation),0);
    if(operation==='has-element') return this.call('completeGlobBoolean',token,request.kind,this.provider.fixture.present.has(uri),0);
    const {elements}=this.provider.elementsAfterAdmission(reader,uri,rule);
    return this.call('completeGlobElements',token,elements,0);
  }
  module(token, request) {
    id(request.id);
    if(request.kind==='select-key') {
      const reader=this.provider.moduleReader(text(request.originalUri));
      if(!reader) {
        if(this.call('completeDefaultModule',token,0)) return;
        // Mirror the finite native Host's final Java GenericUrl factory. The
        // shared engine has already validated this URI before key selection.
        const uri=text(request.originalUri), colon=uri.indexOf(':');
        if(colon>=0 && (uri.slice(colon+1).startsWith('/') || uri.slice(0,colon).toLowerCase()==='jar')) {
          const keyId=id(request.keyId), handle='fixture:java-generic-url';
          must(!this.modules.has(keyId) && !this.genericUrls.has(keyId),'reused GenericUrl module key identity');
          this.genericUrls.set(keyId,{uri,handle});
          return this.call('completeModuleKey',token,keyId,uri,'host',handle,0);
        }
        return this.call('rejectModuleKey',token,false,'no-loader',undefined,0);
      }
      id(request.keyId);must(!this.modules.has(request.keyId) && !this.genericUrls.has(request.keyId),'reused module identity');
      this.modules.set(request.keyId,reader);
      return this.call('completeModuleKey',token,request.keyId,request.originalUri,'host',reader.owner,0);
    }
    if(request.kind==='check-import' || request.keyKind!=='host') {
      must(this.call('completeDefaultModule',token,0),'unconfigured builtin module operation');return;
    }
    if(['query-cache-policy','query-fragment-paths','capabilities'].includes(request.kind) && this.genericUrl(request)) {
      if(request.kind==='query-cache-policy') return this.call('completeModuleCachePolicy',token,request.keyId,true,0);
      if(request.kind==='query-fragment-paths') return this.call('completeModuleFragmentPaths',token,request.keyId,false,0);
      return this.call('completeModuleCapabilities',token,false,true,false,0);
    }
    const reader=this.moduleReader(request), config=this.provider.fixture.config;
    switch(request.kind) {
      case 'query-cache-policy':return this.call('completeModuleCachePolicy',token,request.keyId,true,0);
      case 'query-fragment-paths':
        this.script('fragment-paths',reader.selectedUri);
        return this.call('completeModuleFragmentPaths',token,request.keyId,this.provider.boolean(reader.owner,'fragmentPaths'),0);
      case 'resolve-import-uri': {
        const uri=text(request.importUri), {failure}=this.script('resolve-uri',uri);
        if(failure)return this.call('rejectImportUri',token,request.keyId,failure.kind,failure.message,0);
        this.script('fragment-paths',reader.selectedUri);
        const override=reader.selectedUri===config.get('sourceUri')?config.get('resolveOverride'):null;
        const base=override||reader.selectedUri;
        const resolved=this.call('resolveGlobHostUri',base,uri,this.provider.boolean(reader.owner,'fragmentPaths'));
        return this.call('completeImportUri',token,request.keyId,text(resolved),0);
      }
      case 'capabilities':
        // Java resolveTripleDotImport short-circuits at isLocal=false. No
        // hierarchical/fragment failure script is consumed by this bundled value.
        return this.call('completeModuleCapabilities',token,false,this.provider.boolean(reader.owner,'hierarchical'),this.provider.boolean(reader.owner,'fragmentPaths'),0);
      case 'has-element': {
        const uri=request.originalUri, {failure}=this.script('has-element',uri);
        const denied=failure??this.policy(uri,false);
        if(denied)return this.call('rejectModulePreparation',token,true,denied.kind,denied.message,0);
        return this.call('completeModuleHasElement',token,this.provider.fixture.present.has(uri),0);
      }
      case 'resolve':case 'resolve-key': {
        const uri=reader.selectedUri, {failure}=this.script('resolve',uri);
        const denied=failure??this.policy(uri,false);
        if(denied)return this.call('rejectModuleResolution',token,denied.kind,denied.message,0);
        const override=uri===config.get('sourceUri')?config.get('resolvedPrimary'):null;
        const resolved=override||this.call('normalizeGlobHostUri',uri);
        return this.call('completeModuleResolution',token,uri,text(resolved),resolved,0);
      }
      case 'load-source': {
        const uri=text(request.resolvedUri), response=this.provider.moduleSource(uri);
        const failure=this.failure(response.rule,'load-source');
        if(failure)return this.call('rejectModuleSource',token,failure.kind,failure.message,0);
        const source=response.rule?.kind==='value'?sourceForValue(response.rule.value):response.source;
        return this.call('completeModuleSource',token,text(source),'',0);
      }
      default:throw Error(`unknown module request kind ${request.kind}`);
    }
  }
  read(token) {
    const uri=text(this.call('resolvedReadUri',token));
    const denied=this.policy(uri,true);
    if(denied)return this.call('rejectReadSecurityManager',token,denied.message,0);
    const reader=this.provider.resourceReader(uri.slice(0,uri.indexOf(':')));
    must(reader,'ordinary missing reader representation pending');
    const response=this.provider.resource(uri), rule=response.rule;
    if(rule?.kind==='missing')return this.call('completeReadMissing',token,0);
    const failure=this.failure(rule,'read');
    if(failure) {
      if(failure.kind==='io')return this.call('rejectReadIo',token,failure.message,0);
      if(failure.kind==='security-manager-exception')return this.call('rejectReadSecurityManager',token,failure.message,0);
      throw Error('ordinary external reader error representation pending');
    }
    const value=rule?.kind==='value'?rule.value:response.value;
    return this.call('completeReadText',token,new Uint16Array(codeUnits(text(value))),0);
  }
}

// Returns transport data only. The external bounded capture controller owns file
// creation, Java UTF-8 boundary encoding and strict raw oracle comparisons.
export function runGlobFixture(SourceSession, files, profile) {
  const fixture=parseGlobFixture(files);
  const session=SourceSession.moduleEvaluator('compact',true,false,false);
  const host=new GlobFixtureHost(session,fixture), rows=[];
  let failed=null;
  try {
    must(profile==='production16384'||profile==='explicit512','explicit threshold profile required');
    if(profile==='explicit512')session.setGlobTestMode(true);
    for(const query of fixture.queries) {
      const row={id:query.id,expression:codeUnits(query.expression),logs:[]};rows.push(row);
      if(failed){row.status='unexecuted\n';row.adapterError=codeUnits(failed);continue;}
      host.provider.phase=`query:${query.id}`;
      try {
        const contents=fixture.config.get('primaryMode')==='provider'?undefined:fixture.source;
        session.evaluateModuleSource(text(fixture.config.get('sourceUri')),contents,query.expression,0,undefined);
        let terminal=false;
        for(let step=0;step<1_000_000;step++) {
          const event=session.poll(0,4096);
          if(event.kind==='yield')continue;
          host.effects.push({phase:host.provider.phase,direction:'request',event:copy(event)});
          switch(event.kind) {
            case 'module-request':host.module(text(event.token),event.module);break;
            case 'glob-request':host.glob(text(event.token),event.glob);break;
            case 'read':host.read(text(event.token));break;
            case 'log':row.logs.push(copy(event.log));host.call('acknowledgeLog',text(event.token),undefined,0);break;
            case 'value':row.status='success\n';row.stdout=Array.from(event.value);row.stderr=[];terminal=true;break;
            case 'error':row.status=event.disposition==='unsupported'?'unsupported\n':'error\n';row.stdout=[];row.stderr=Array.from(event.error);row.disposition=event.disposition;terminal=true;break;
            default:throw Error(`unknown source event ${event.kind}`);
          }
          if(terminal)break;
        }
        must(terminal,'finite driver poll bound exceeded');
      } catch(error) {
        failed=String(error?.message??error);row.status='adapter-error\n';row.adapterError=codeUnits(failed);
      }
    }
  } finally {session.close();}
  return {complete:failed===null,profile,rows,effects:host.effects,fixtureDecisions:host.provider.decisions,
    provenance:'actual Rust typed binding effects; not Java SPI callbacks',fullCompatibilityClaim:false};
}
