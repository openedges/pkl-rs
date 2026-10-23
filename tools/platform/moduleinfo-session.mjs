// Thin browser-safe SourceSession transport; no ModuleInfo/constraint/version semantics.
function units(value) {
  if (!(value instanceof Uint16Array)) throw Error('expected Uint16Array');
  return Array.from(value);
}
function textUnits(value) {
  if (typeof value !== 'string') throw Error('expected error message string');
  return Array.from({length:value.length},(_,index)=>value.charCodeAt(index));
}
export function runModuleInfoCase(api, fixture) {
  let session,polls=0;
  const retained=[],result={id:fixture.id,repeat:fixture.repeat,scope:fixture.comparisonScope,calls:[],constructorError:null,acquisitionError:null,architecture:{collections:0,terminalSlots:null,ownedResultsStable:true}};
  try {
    try {session=api.SourceSession.forExpressions(fixture.source,fixture.moduleUri,fixture.moduleName,fixture.expressions,'compact',false,false,true);}
    catch(error) {result.constructorError={isError:error instanceof Error,messageUnits:textUnits(error.message)};return result;}
    for(let index=0;index<fixture.expressions.length;index++) {
      session.startPath(index,0);
      while(true) {
        if(++polls>1000000)throw Error('ModuleInfo poll bound exceeded');
        const event=session.poll(0,1);session.collect();result.architecture.collections++;
        if(event.kind==='yield')continue;
        if(!['value','error'].includes(event.kind))throw Error('unexpected ModuleInfo event '+event.kind);
        const view=event.kind==='value'?event.value:event.error,data=units(view);
        retained.push({view,data});result.calls.push({prefix:`outer-${index}`,kind:event.kind,units:data,phase:event.phase??null,disposition:event.disposition??null,reason:event.reason??null});
        break;
      }
    }
  } catch(error) {result.acquisitionError={message:String(error.message??error),stack:error.stack??null};}
  finally {
    if(session)try {session.close();session.collect();result.architecture.terminalSlots=session.heapSlots();if(typeof session.free==='function')session.free();result.architecture.ownedResultsStable=retained.every(({view,data})=>view.length===data.length&&data.every((unit,index)=>view[index]===unit));}
    catch(error) {result.acquisitionError={message:String(error.message??error),stack:error.stack??null};}
  }
  return result;
}
