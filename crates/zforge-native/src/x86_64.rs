use std::mem::{offset_of, transmute};

use cranelift_codegen::{
    ir::{AbiParam, InstBuilder, MemFlags, Type, Value, condcodes::IntCC, types},
    isa::OwnedTargetIsa,
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module, default_libcall_names};
use zforge_core::{Cpu, FlatMemory, Memory, flags};
use zforge_decoder::{Condition, Decoded, Instruction, Operand8, Register16, decode_with};

use crate::NativeError;

const MAX_BLOCK_INSTRUCTIONS: u32 = 64;

/// Compiles a fresh block per dispatch. Generated code and its allocation live
/// only for the duration of the call.
pub struct NativeEngine {
    isa: OwnedTargetIsa,
}

impl NativeEngine {
    pub fn new() -> Result<Self, NativeError> {
        let mut flags = settings::builder();
        flags
            .set("use_colocated_libcalls", "false")
            .map_err(codegen)?;
        flags.set("is_pic", "false").map_err(codegen)?;
        let isa = cranelift_native::builder()
            .map_err(codegen)?
            .finish(settings::Flags::new(flags))
            .map_err(codegen)?;
        Ok(Self { isa })
    }

    /// Run up to one block of guest instructions and return the number retired.
    /// A branch, HALT, guest write, or the instruction budget ends the block.
    /// Unsupported instructions end a supported block before the offending PC,
    /// and return a diagnostic when that PC is dispatched.
    /// A zero budget or an already halted CPU returns zero without compiling.
    pub fn run_block(
        &mut self,
        cpu: &mut Cpu,
        memory: &mut FlatMemory,
        budget: u32,
    ) -> Result<u32, NativeError> {
        if cpu.halted || budget == 0 {
            return Ok(0);
        }

        let mut allocation = ExecutableMemory(Some(JITModule::new(JITBuilder::with_isa(
            self.isa.clone(),
            default_libcall_names(),
        ))));
        let module = allocation.0.as_mut().expect("module is owned until drop");
        let mut context = module.make_context();
        let pointer_type = module.target_config().pointer_type();
        context
            .func
            .signature
            .params
            .push(AbiParam::new(pointer_type));
        let mut builder_context = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let frame_pointer = builder.block_params(entry)[0];
        let mut emitter = Emitter {
            builder,
            frame_pointer,
        };
        let mut instructions = 0;
        let mut writes = 0;
        let mut pc = cpu.pc;
        for _ in 0..budget.min(MAX_BLOCK_INSTRUCTIONS) {
            let decoded = match decode_with(pc, |address| memory.peek(address)) {
                Ok(decoded) => decoded,
                Err(error) if instructions == 0 => return Err(NativeError::Decode(error)),
                Err(_) => break,
            };
            let Some(flow) = emitter.instruction(decoded) else {
                if instructions == 0 {
                    return Err(NativeError::UnsupportedInstruction {
                        pc,
                        opcode: memory.peek(pc),
                    });
                }
                break;
            };
            pc = decoded.next_pc;
            instructions += 1;
            match flow {
                Flow::Next => {}
                Flow::Stop => break,
                Flow::Write(count) => {
                    writes = count;
                    break;
                }
            }
        }
        emitter.finish(instructions);
        let id = module
            .declare_anonymous_function(&context.func.signature)
            .map_err(codegen)?;
        module.define_function(id, &mut context).map_err(codegen)?;
        module.finalize_definitions().map_err(codegen)?;

        let mut frame = Frame::from(&*cpu);
        run_generated(module, id, &mut frame);
        frame.apply(cpu);
        for write in &frame.writes[..writes] {
            memory.write(write.address, write.value);
        }
        Ok(instructions)
    }
}

fn codegen(error: impl std::fmt::Display) -> NativeError {
    NativeError::Codegen(error.to_string())
}

struct ExecutableMemory(Option<JITModule>);

impl Drop for ExecutableMemory {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if let Some(module) = self.0.take() {
            // SAFETY: The sole call is synchronous and no code pointer escapes
            // run_generated. There can be no outstanding call into this module.
            unsafe { module.free_memory() };
        }
    }
}

#[allow(unsafe_code)]
fn run_generated(module: &JITModule, id: cranelift_module::FuncId, frame: &mut Frame) {
    // SAFETY: The finalized function was emitted with the host C calling
    // convention, one pointer argument, and no return value. Its only memory
    // operations are at fixed offsets into the exclusively borrowed Frame.
    // The module retains the executable allocation for the whole call.
    unsafe {
        let function: unsafe extern "C" fn(*mut Frame) =
            transmute(module.get_finalized_function(id));
        function(frame);
    }
}

#[repr(C)]
struct Frame {
    t_states: u64,
    pc: u16,
    sp: u16,
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    r: u8,
    halted: u8,
    // Writers end a block. Only CALL emits two writes.
    writes: [PendingWrite; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PendingWrite {
    address: u16,
    value: u8,
}

impl From<&Cpu> for Frame {
    fn from(cpu: &Cpu) -> Self {
        let [b, c] = cpu.main.bc.to_be_bytes();
        let [d, e] = cpu.main.de.to_be_bytes();
        let [h, l] = cpu.main.hl.to_be_bytes();
        Self {
            t_states: cpu.t_states,
            pc: cpu.pc,
            sp: cpu.sp,
            a: cpu.main.a,
            f: cpu.main.f,
            b,
            c,
            d,
            e,
            h,
            l,
            r: cpu.r,
            halted: u8::from(cpu.halted),
            writes: [PendingWrite::default(); 2],
        }
    }
}

impl Frame {
    fn apply(&self, cpu: &mut Cpu) {
        cpu.t_states = self.t_states;
        cpu.pc = self.pc;
        cpu.sp = self.sp;
        cpu.main.a = self.a;
        cpu.main.f = self.f;
        cpu.main.bc = u16::from_be_bytes([self.b, self.c]);
        cpu.main.de = u16::from_be_bytes([self.d, self.e]);
        cpu.main.hl = u16::from_be_bytes([self.h, self.l]);
        cpu.r = self.r;
        cpu.halted = self.halted != 0;
    }
}

enum Flow {
    Next,
    Stop,
    Write(usize),
}

struct Emitter<'a> {
    builder: FunctionBuilder<'a>,
    frame_pointer: Value,
}

impl Emitter<'_> {
    fn instruction(&mut self, decoded: Decoded) -> Option<Flow> {
        let mut next_pc = self.constant(types::I16, decoded.next_pc);
        let (cycles, flow) = match decoded.instruction {
            Instruction::Nop => (4, Flow::Next),
            Instruction::Halt => {
                self.store_immediate(types::I8, 1_u8, offset_of!(Frame, halted));
                (4, Flow::Stop)
            }
            Instruction::LoadImmediate8(register, value) => {
                self.store_immediate(types::I8, value, register_offset(register)?);
                (7, Flow::Next)
            }
            Instruction::LoadImmediate16(register, value) => {
                self.load_immediate_16(register, value);
                (10, Flow::Next)
            }
            Instruction::Decrement(register) => {
                self.decrement(register_offset(register)?);
                (4, Flow::Next)
            }
            Instruction::StoreA(address) => {
                let address = self.constant(types::I16, address);
                let a = self.load(types::I8, offset_of!(Frame, a));
                self.guest_write(0, address, a);
                (13, Flow::Write(1))
            }
            Instruction::JumpRelative(condition, displacement) => {
                let target = decoded.next_pc.wrapping_add_signed(i16::from(displacement));
                let target = self.constant(types::I16, target);
                if condition == Condition::Always {
                    next_pc = target;
                    (12, Flow::Stop)
                } else {
                    let taken = self.condition(condition);
                    next_pc = self.builder.ins().select(taken, target, next_pc);
                    let taken_cycles = self.constant(types::I64, 5_u8);
                    let zero = self.constant(types::I64, 0_u8);
                    let additional = self.builder.ins().select(taken, taken_cycles, zero);
                    self.add_cycles(additional);
                    (7, Flow::Stop)
                }
            }
            Instruction::Jump(address) => {
                next_pc = self.constant(types::I16, address);
                (10, Flow::Stop)
            }
            Instruction::Call(address) => {
                self.push_return_address(decoded.next_pc);
                next_pc = self.constant(types::I16, address);
                (17, Flow::Write(2))
            }
            _ => return None,
        };
        self.store(next_pc, offset_of!(Frame, pc));
        let cycles = self.constant(types::I64, cycles);
        self.add_cycles(cycles);
        Some(flow)
    }

    fn load_immediate_16(&mut self, register: Register16, value: u16) {
        let (high, low) = match register {
            Register16::Bc => (offset_of!(Frame, b), offset_of!(Frame, c)),
            Register16::De => (offset_of!(Frame, d), offset_of!(Frame, e)),
            Register16::Hl => (offset_of!(Frame, h), offset_of!(Frame, l)),
            Register16::Sp => {
                self.store_immediate(types::I16, value, offset_of!(Frame, sp));
                return;
            }
        };
        let [high_value, low_value] = value.to_be_bytes();
        self.store_immediate(types::I8, high_value, high);
        self.store_immediate(types::I8, low_value, low);
    }

    fn decrement(&mut self, offset: usize) {
        let previous = self.load(types::I8, offset);
        let result = self.builder.ins().iadd_imm(previous, -1);
        let previous_flags = self.load(types::I8, offset_of!(Frame, f));
        let carry = self
            .builder
            .ins()
            .band_imm(previous_flags, i64::from(flags::C));
        let result_bits = i64::from(flags::S | flags::Y | flags::X);
        let sign_xy = self.builder.ins().band_imm(result, result_bits);
        let zero = self.flag_if_equal(result, 0, flags::Z);
        let low_nibble = self.builder.ins().band_imm(previous, 0x0f);
        let half_borrow = self.flag_if_equal(low_nibble, 0, flags::H);
        let overflow = self.flag_if_equal(previous, 0x80, flags::PV);
        let mut flags = self.builder.ins().bor_imm(carry, i64::from(flags::N));
        for bit in [sign_xy, zero, half_borrow, overflow] {
            flags = self.builder.ins().bor(flags, bit);
        }
        self.store(flags, offset_of!(Frame, f));
        self.store(result, offset);
    }

    fn flag_if_equal(&mut self, value: Value, expected: i64, flag: u8) -> Value {
        let equal = self.builder.ins().icmp_imm(IntCC::Equal, value, expected);
        let flag = self.constant(types::I8, flag);
        let zero = self.constant(types::I8, 0_u8);
        self.builder.ins().select(equal, flag, zero)
    }

    fn condition(&mut self, condition: Condition) -> Value {
        let (test, flag) = match condition {
            Condition::NotZero => (IntCC::Equal, flags::Z),
            Condition::Zero => (IntCC::NotEqual, flags::Z),
            Condition::NotCarry => (IntCC::Equal, flags::C),
            Condition::Carry => (IntCC::NotEqual, flags::C),
            Condition::Always => unreachable!("unconditional jump handled separately"),
        };
        let flags = self.load(types::I8, offset_of!(Frame, f));
        let masked = self.builder.ins().band_imm(flags, i64::from(flag));
        self.builder.ins().icmp_imm(test, masked, 0)
    }

    fn push_return_address(&mut self, address: u16) {
        let sp = self.load(types::I16, offset_of!(Frame, sp));
        let high_address = self.builder.ins().iadd_imm(sp, -1);
        let low_address = self.builder.ins().iadd_imm(sp, -2);
        let [high, low] = address.to_be_bytes();
        let high = self.constant(types::I8, high);
        let low = self.constant(types::I8, low);
        self.guest_write(0, high_address, high);
        self.guest_write(1, low_address, low);
        self.store(low_address, offset_of!(Frame, sp));
    }

    fn guest_write(&mut self, index: usize, address: Value, value: Value) {
        let base = offset_of!(Frame, writes) + index * size_of::<PendingWrite>();
        self.store(address, base + offset_of!(PendingWrite, address));
        self.store(value, base + offset_of!(PendingWrite, value));
    }

    fn finish(mut self, instruction_count: u32) {
        let previous_r = self.load(types::I8, offset_of!(Frame, r));
        let bit_seven = self.builder.ins().band_imm(previous_r, 0x80);
        let advanced = self
            .builder
            .ins()
            .iadd_imm(previous_r, i64::from(instruction_count));
        let low_seven = self.builder.ins().band_imm(advanced, 0x7f);
        let r = self.builder.ins().bor(bit_seven, low_seven);
        self.store(r, offset_of!(Frame, r));
        self.builder.ins().return_(&[]);
        self.builder.finalize();
    }

    fn add_cycles(&mut self, cycles: Value) {
        let total = self.load(types::I64, offset_of!(Frame, t_states));
        let next = self.builder.ins().iadd(total, cycles);
        self.store(next, offset_of!(Frame, t_states));
    }

    fn load(&mut self, ty: Type, offset: usize) -> Value {
        self.builder
            .ins()
            .load(ty, MemFlags::new(), self.frame_pointer, offset as i32)
    }

    fn store(&mut self, value: Value, offset: usize) {
        self.builder
            .ins()
            .store(MemFlags::new(), value, self.frame_pointer, offset as i32);
    }

    fn constant(&mut self, ty: Type, value: impl Into<i64>) -> Value {
        self.builder.ins().iconst(ty, value.into())
    }

    fn store_immediate(&mut self, ty: Type, value: impl Into<i64>, offset: usize) {
        let value = self.constant(ty, value);
        self.store(value, offset);
    }
}

fn register_offset(operand: Operand8) -> Option<usize> {
    match operand {
        Operand8::A => Some(offset_of!(Frame, a)),
        Operand8::B => Some(offset_of!(Frame, b)),
        Operand8::C => Some(offset_of!(Frame, c)),
        Operand8::D => Some(offset_of!(Frame, d)),
        Operand8::E => Some(offset_of!(Frame, e)),
        Operand8::H => Some(offset_of!(Frame, h)),
        Operand8::L => Some(offset_of!(Frame, l)),
        Operand8::AtHl => None,
    }
}
