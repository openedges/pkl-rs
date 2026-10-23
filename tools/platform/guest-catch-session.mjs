// Browser-safe Host transport only: Pkl parsing, catch, caches and unwind stay in Rust.
import {completeDefaultModuleRequest, completeFixtureModuleKey, validateModuleRequest} from './module-policy-session.mjs';
function units(value) {
  if (!(value instanceof Uint16Array)) throw Error('expected Uint16Array');
  return Array.from(value);
}
function textUnits(value) {
  if (typeof value !== 'string') throw Error('expected string');
  return Array.from({length:value.length},(_,i)=>value.charCodeAt(i));
}
function asString(value) {
  let result='';
  for(let i=0;i<value.length;i+=4096) result+=String.fromCharCode(...value.slice(i,i+4096));
  return result;
}
export function runGuestCatchCase(api, fixture) {
  let session, callbacksDone=false, polls=0;
  const moduleUris=new Set(['catch:/child']);
  const calls=[],events=[],logs=[],retained=[];
  const counts={resolve:0,source:0,read:0,log:0,callback:0,event:0};
  const architecture={collections:0,terminalSlots:null,ownedResultsStable:true,cancelledReads:[],cancelledModules:[]};
  const result={group:fixture.group,id:fixture.id,repeat:fixture.repeat,scope:fixture.comparisonScope,calls,events,logs,counts,architecture,moduleRequests:[],modulePolicy:'explicit-catch-host-with-pkl-0.32.0-default-import-policy',constructorError:null,acquisitionError:null};
  const emit=value=>{events.push(value);counts.event++;};
  function complete(event,prefix) {
    if(!['value','error'].includes(event.kind))throw Error('unexpected completion '+event.kind);
    const view=event.kind==='value'?event.value:event.error,data=units(view);
    retained.push({view,data});calls.push({prefix,kind:event.kind,units:data,disposition:event.disposition??null});
    emit(prefix+(event.kind==='value'?' success':' error org.pkl.core.PklException'));
  }
  function callback(token) {
    if(callbacksDone)return;
    callbacksDone=true;emit('callback-start');
    for(let i=0;i<fixture.callbacks.length;i++) {
      session.reenterPath(token,fixture.expressions.length+i,0);
      complete(settle(),`inner-${counts.callback++}`);
    }
    emit('callback-end');
  }
  function next(now=0) {
    while(++polls<=1000000) {
      const event=session.poll(now,1);session.collect();architecture.collections++;
      if(event.kind==='module-request'){
        const request=event.module;
        result.moduleRequests.push({token:event.token,module:{...request}});
        if(['select-key','query-cache-policy','query-fragment-paths','resolve-import-uri'].includes(request.kind)){
          if(!completeFixtureModuleKey(session,event,moduleUris)&&!completeDefaultModuleRequest(session,event))
            throw Error('unsupported guest-catch module key');
          continue;
        }
        if(request.originalUri==='catch:/child')validateModuleRequest(event);
        if(request.kind==='check-import'||(request.kind==='resolve'&&request.originalUri!=='catch:/child')){
          if(!completeDefaultModuleRequest(session,event))throw Error('unsupported guest-catch module request');
          continue;
        }
      }
      if(event.kind!=='yield')return event;
    }
    throw Error('poll bound exceeded');
  }
  function settle() {
    while(true) {
      const event=next(),token=event.token;
      if(event.kind==='module-request') {
        const request=event.module;
        if(request.originalUri!=='catch:/child')throw Error('unexpected module URI');
        if(request.kind==='resolve') {
          emit(`resolve ${request.originalUri} attempt=${++counts.resolve}`);
          if(fixture.mode==='resolve-retry'&&counts.resolve===1) session.rejectModuleResolution(token,'io','resolve-first',0);
          else session.completeModuleResolution(token,'catch:/child','catch:/child','catch:/child',0);
        } else if(request.kind==='load-source') {
          const attempt=++counts.source;emit(`source-start ${request.originalUri} attempt=${attempt}`);
          if(fixture.mode==='source-reentry'&&attempt===1)callback(token);
          if(['source-cache','source-reentry'].includes(fixture.mode)&&attempt===1)session.rejectModuleSource(token,'io','source-first',0);
          else {emit(`source-end ${request.originalUri} attempt=${attempt}`);session.completeModuleSource(token,'value = 9\n','child',0);}
        } else throw Error('unexpected module request kind');
      } else if(event.kind==='read') {
        emit(`read ${event.uri} attempt=${++counts.read}`);
        if(fixture.mode==='read-reentry'&&counts.read===1)callback(token);
        if(['read-retry','read-reentry'].includes(fixture.mode)&&counts.read===1) session.rejectReadIo(token,'read-first',0);
        else {
          let value=[57];
          if(fixture.mode==='renderer') {
            const name={'catch:/units':'payload-units','catch:/white':'payload-white','catch:/blank':'payload-blank'}[event.uri];
            if(!name)throw Error('unexpected renderer read');value=fixture.payloadUnits[name];
          }
          session.respondString(token,Uint16Array.from(value),0);
        }
      } else if(event.kind==='log') {
        const log=event.log;
        if(!Object.hasOwn(log,'memberName'))throw Error('missing nullable logger member');
        logs.push({level:log.level,message:units(log.message),moduleUri:units(log.moduleUri),memberName:log.memberName===null?null:units(log.memberName),sourceLines:log.sourceLines.map(units),startLine:log.startLine,startColumn:log.startColumn,endLine:log.endLine,endColumn:log.endColumn});
        counts.log++;emit('trace '+asString(logs.at(-1).message));
        if(fixture.mode==='logger-reentry')callback(token);
        session.acknowledgeLog(token,undefined,0);
      } else if(['value','error'].includes(event.kind))return event;
      else throw Error('unexpected event '+event.kind);
    }
  }
  try {
    try {session=api.SourceSession.forExpressions(fixture.source,fixture.moduleUri,'main',[...fixture.expressions,...fixture.callbacks],'compact',true,false,true);}
    catch(error) {result.constructorError={isError:error instanceof Error,messageUnits:textUnits(error.message)};return result;}
    if(fixture.comparisonScope==='frontend-constructor-only')throw Error('invalid authored source unexpectedly accepted');
    if(fixture.comparisonScope==='terminal-partial-parity') {
      const timeout=fixture.mode.endsWith('-timeout')?3000:undefined;
      session.startPath(0,0,timeout);complete(next(),'outer-0');
      session.startPath(1,0,timeout);let event=next();
      if(event.kind==='module-request'&&event.module.kind==='resolve') {session.completeModuleResolution(event.token,'catch:/child','catch:/child','catch:/child',0);event=next();}
      if(!['read','module-request'].includes(event.kind))throw Error('terminal fixture did not reach Host request');
      if(timeout!==undefined) {
        const failure=next(3001);result.terminalFailure={kind:failure.kind,disposition:failure.disposition,errorUnits:units(failure.error)};
        architecture.cancelledReads=session.drainCancelledReads();architecture.cancelledModules=session.drainCancelledModules();
        result.pendingToken=event.token;
      } else {
        session.closeFromCallback(event.token);
        if(event.kind==='read')session.respondString(event.token,Uint16Array.from([57]),0);
        else session.completeModuleSource(event.token,'value = 9\n','child',0);
        complete(next(),'outer-1');
      }
      result.followupRejections=[];
      for(const index of [2,3]) {try {session.startPath(index,3001);result.followupRejections.push(false);}catch {result.followupRejections.push(true);}}
    } else for(let i=0;i<fixture.expressions.length;i++) {session.startPath(i,0);complete(settle(),`outer-${i}`);}
  } catch(error) {result.acquisitionError={message:String(error.message??error),stack:error.stack??null};}
  finally {
    if(session)try {session.close();session.collect();architecture.terminalSlots=session.heapSlots();if(typeof session.free==='function')session.free();architecture.ownedResultsStable=retained.every(({view,data})=>view.length===data.length&&data.every((v,i)=>view[i]===v));}
    catch(error) {result.acquisitionError={message:String(error.message??error),stack:error.stack??null};}
  }
  return result;
}
