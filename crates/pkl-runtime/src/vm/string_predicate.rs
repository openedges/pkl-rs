//! Predicate continuations use ordinary traced VM stack values and call frames.
use super::*;
impl Vm {
    pub(super) fn string_predicate_begin(
        &mut self,
        text: &PklString,
        last: bool,
    ) -> Result<(), RuntimeError> {
        // The checked callback remains on the stack across child calls. Bytes are
        // an internal immutable UTF-16 cursor backing, never exposed to Pkl.
        let bytes = text
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>();
        let cursor = if last { bytes.len() / 2 } else { 0 };
        self.stack.push(Value::Bytes(bytes.into()));
        self.stack.push(Value::Int((cursor as i64).into()));
        self.stack.push(Value::Int(0.into()));
        Ok(())
    }
    fn string_predicate_state(&self, last: bool) -> Result<(Value, Vec<u16>, usize), RuntimeError> {
        let n = self.stack.len();
        let [
            callback,
            Value::Bytes(bytes),
            Value::Int(cursor),
            Value::Int(_),
        ] = self
            .stack
            .get(n.saturating_sub(4)..)
            .ok_or_else(|| invalid("String predicate state"))?
        else {
            return Err(invalid("String predicate state types"));
        };
        let cursor = cursor.get() as usize;
        let unit = |index: usize| u16::from_be_bytes([bytes[2 * index], bytes[2 * index + 1]]);
        let mut units = Vec::with_capacity(2);
        if last && cursor > 0 {
            let trailing = unit(cursor - 1);
            if (0xdc00..=0xdfff).contains(&trailing)
                && cursor > 1
                && (0xd800..=0xdbff).contains(&unit(cursor - 2))
            {
                units.push(unit(cursor - 2));
            }
            units.push(trailing);
        } else if !last && cursor < bytes.len() / 2 {
            let leading = unit(cursor);
            units.push(leading);
            if (0xd800..=0xdbff).contains(&leading)
                && cursor + 1 < bytes.len() / 2
                && (0xdc00..=0xdfff).contains(&unit(cursor + 1))
            {
                units.push(unit(cursor + 1));
            }
        }
        Ok((callback.clone(), units, cursor))
    }
    fn string_predicate_finish(
        &mut self,
        text: PklString,
        last: bool,
        drop: bool,
    ) -> Result<(), RuntimeError> {
        let Value::Int(count) = self.pop()? else {
            return Err(invalid("String predicate count"));
        };
        self.pop()?;
        self.pop()?;
        self.pop()?;
        let length = text.code_point_len() as i64;
        let count = count.get();
        let (start, end) = match (last, drop) {
            (false, false) => (0, count),
            (true, false) => (length - count, length),
            (false, true) => (count, length),
            (true, true) => (0, length - count),
        };
        self.stack.push(Value::String(
            text.substring_code_points(start, end)
                .expect("predicate bounds"),
        ));
        self.frames
            .last_mut()
            .ok_or_else(|| invalid("String predicate frame"))?
            .pc = 6;
        Ok(())
    }
    pub(super) fn string_predicate_next(
        &mut self,
        text: PklString,
        last: bool,
        drop: bool,
    ) -> Result<(), RuntimeError> {
        let (callback, units, _) = self.string_predicate_state(last)?;
        if units.is_empty() {
            return self.string_predicate_finish(text, last, drop);
        }
        self.stack.push(callback);
        self.stack.push(Value::String(PklString::from_utf16(units)));
        Ok(())
    }
    pub(super) fn string_predicate_accept(
        &mut self,
        text: PklString,
        last: bool,
        drop: bool,
    ) -> Result<(), RuntimeError> {
        let result = self.pop()?;
        let Value::Bool(accepted) = result else {
            return Err(type_error(
                &TypeCheck::Boolean,
                &result,
                &self.program.module_name,
                &self.heap,
            )?);
        };
        if !accepted {
            return self.string_predicate_finish(text, last, drop);
        }
        let (_, units, cursor) = self.string_predicate_state(last)?;
        let cursor = if last {
            cursor - units.len()
        } else {
            cursor + units.len()
        };
        let n = self.stack.len();
        let Value::Int(count) = &self.stack[n - 1] else {
            return Err(invalid("predicate count"));
        };
        let count = count.get();
        self.stack[n - 2] = Value::Int((cursor as i64).into());
        self.stack[n - 1] = Value::Int((count + 1).into());
        self.frames
            .last_mut()
            .ok_or_else(|| invalid("String predicate frame"))?
            .pc = 3;
        Ok(())
    }
}
