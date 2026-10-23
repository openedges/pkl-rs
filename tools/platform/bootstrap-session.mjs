// Thin controlled property Host. The shared engine owns all bootstrap/cache/renderer behavior.
function units(view){if(!(view instanceof Uint16Array))throw Error('expected Uint16Array');return Array.from(view);}
function textUnits(text){if(typeof text!=='string')throw Error('expected error message');return Array.from({length:text.length},(_,i)=>text.charCodeAt(i));}
export function runBootstrapCase(api,fixture){
 const result={id:fixture.id,repeat:fixture.repeat,sessions:[]};
 for(let index=0;index<fixture.formats.length;index++){
  const format=fixture.formats[index],entry={index,format,calls:[],constructorError:null,acquisitionError:null,reads:0,architecture:{terminalSlots:null,ownedResultsStable:true,collections:0}},retained=[];result.sessions.push(entry);let session,polls=0;
  try{
   try{session=api.SourceSession.forExpressions(fixture.source,fixture.moduleUri,'main',fixture.expressions,'compact',false,false,true);}catch(error){entry.constructorError={isError:error instanceof Error,messageUnits:textUnits(error.message)};continue;}
   for(let call=0;call<fixture.expressions.length;call++){
    const readsBefore=entry.reads;session.startPath(call,0);
    while(true){
     if(++polls>1000000)throw Error('bootstrap poll bound exceeded');
     const event=session.poll(0,1);session.collect();entry.architecture.collections++;
     if(event.kind==='yield')continue;
     if(event.kind==='read'){
      entry.reads++;if(event.uri!=='prop:pkl.outputFormat')throw Error('unexpected bootstrap resource '+event.uri);
      if(format==='absent')session.respondConfigured(event.token,0);else session.respondString(event.token,Uint16Array.from(textUnits(format)),0);
      continue;
     }
     if(!['value','error'].includes(event.kind))throw Error('unexpected bootstrap event '+event.kind);
     const view=event.kind==='value'?event.value:event.error,data=units(view);retained.push({view,data});entry.calls.push({prefix:`outer-${call}`,kind:event.kind,units:data,phase:event.phase??null,disposition:event.disposition??null,reason:event.reason??null,readsBefore,readsAfter:entry.reads});break;
    }
   }
  }catch(error){entry.acquisitionError={message:String(error.message??error),stack:error.stack??null};}
  finally{if(session)try{session.close();session.collect();entry.architecture.terminalSlots=session.heapSlots();if(typeof session.free==='function')session.free();entry.architecture.ownedResultsStable=retained.every(({view,data})=>view.length===data.length&&data.every((u,i)=>view[i]===u));}catch(error){entry.acquisitionError={message:String(error.message??error),stack:error.stack??null};}}
 }
 return result;
}
