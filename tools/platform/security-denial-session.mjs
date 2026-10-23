// Typed Java SecurityManagerException provenance only; generic Denied/raw Host rejection is distinct.
import {completeDefaultModuleRequest} from './module-policy-session.mjs';
function units(view){if(!(view instanceof Uint16Array))throw Error('expected Uint16Array');return Array.from(view);}
function fromUnits(value){let text='';for(let i=0;i<value.length;i+=4096)text+=String.fromCharCode(...value.slice(i,i+4096));return text;}
export function runSecurityDenialCase(api,fixture){
 let session,polls=0;const retained=[],result={id:fixture.id,repeat:fixture.repeat,calls:[],denials:[],moduleRequests:[],modulePolicy:'explicit-resolve-denial-with-pkl-0.32.0-default-import-policy',constructorError:null,acquisitionError:null,architecture:{terminalSlots:null,ownedResultsStable:true,collections:0}};
 try{
  try{session=api.SourceSession.forExpressions(fixture.source,fixture.moduleUri,'main',fixture.expressions,'compact',false,false,true);}catch(error){result.constructorError={isError:error instanceof Error,messageUnits:typeof error.message==='string'?Array.from({length:error.message.length},(_,i)=>error.message.charCodeAt(i)):null};return result;}
  for(let index=0;index<fixture.expressions.length;index++){
   session.startPath(index,0);
   while(true){
    if(++polls>1000000)throw Error('security denial poll bound exceeded');const event=session.poll(0,1);session.collect();result.architecture.collections++;
    if(event.kind==='yield')continue;
    if(event.kind==='module-request'){
     const request=event.module,cause=fixture.causes[result.denials.length];
     result.moduleRequests.push({callIndex:index,token:event.token,module:{...request}});
     if(request.kind==='check-import'){
      if(!completeDefaultModuleRequest(session,event))throw Error('unsupported security import check');
      continue;
     }
     if(request.kind!=='resolve'||request.originalUri!==fixture.deniedUri||!cause||cause.errorClass!=='org.pkl.core.SecurityManagerException'||cause.uri!==request.originalUri)throw Error('typed security resolve/cause contract mismatch');
     result.denials.push({...request,token:event.token});session.rejectModuleResolution(event.token,'security-manager-exception',fromUnits(cause.units),0);continue;
    }
    if(!['value','error'].includes(event.kind))throw Error('unexpected security event '+event.kind);
    const view=event.kind==='value'?event.value:event.error,data=units(view);retained.push({view,data});result.calls.push({prefix:`outer-${index}`,kind:event.kind,units:data,disposition:event.disposition??null,phase:event.phase??null,reason:event.reason??null,denialsAfter:result.denials.length});break;
   }
  }
 }catch(error){result.acquisitionError={message:String(error.message??error),stack:error.stack??null};}
 finally{if(session)try{session.close();session.collect();result.architecture.terminalSlots=session.heapSlots();if(typeof session.free==='function')session.free();result.architecture.ownedResultsStable=retained.every(({view,data})=>view.length===data.length&&data.every((v,i)=>view[i]===v));}catch(error){result.acquisitionError={message:String(error.message??error),stack:error.stack??null};}}
 return result;
}
