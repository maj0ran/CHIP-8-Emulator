extern crate termion;

use std::fs::File;
use std::io::Read;
use std::io::{self, Write};
use std::{thread, time};
use termion::terminal_size;
use termion::{clear, color, cursor};
use std::sync::Mutex;

trait ToBits {
    fn to_bits(&self) -> [bool; 8];
}

impl ToBits for u8 {
    fn to_bits(&self) -> [bool; 8] {
        let mut bitarray: [bool; 8] = [false; 8];
        for i in 0..=7 {
            let bitval = (self >> i) & 0x01;
            let bitval = if bitval == 0 { false } else { true };
            bitarray[7 - i] = bitval;
        }
        bitarray
    }
}

struct CPU {
    pc: usize,
    regs: [u8; 16],
    memory: [u8; 0x1000],
    stack: [u16; 16],
    sp: usize,
    index: u16,
    delay_timer: u8,
    sound_timer: u8,
    keyboard: [bool; 16],
    display: [[bool; 64]; 32],
}

impl CPU {
    fn new() -> CPU {
        CPU {
            pc: 0x200,
            regs: [0; 16],
            memory: [0; 0x1000],
            stack: [0; 16],
            sp: 0,
            index: 0,
            delay_timer: 0,
            sound_timer: 0,
            keyboard: [false; 16],
            display: [[false; 64]; 32],
        }
    }

    fn next(&mut self) -> u16 {
        let pc = self.pc;
        self.pc += 2;

        let op_byte1 = self.memory[pc] as u16;
        let op_byte2 = self.memory[pc + 1] as u16;
        //      println!("PC: {}; {:x} {:x}", pc, op_byte1, op_byte2);
        (op_byte1 << 8) | op_byte2
    }

    fn run(&mut self) {
        loop {
            let opcode = self.next();

//            thread::spawn(|| { self.delay_timer -= 1; });
            //       println!("OP: {:x}", opcode);
            if self.delay_timer > 0 {
                self.delay_timer -= 1;
            }
            if self.sound_timer > 0 {
                self.sound_timer -= 1;
            }
            let c = ((opcode & 0xF000) >> 12) as u8;
            let x = ((opcode & 0x0F00) >> 8) as u8;
            let y = ((opcode & 0x00F0) >> 4) as u8;
            let d = ((opcode & 0x000F) >> 0) as u8;

            let nnn = opcode & 0xFFF;
            let kk = (opcode & 0x00FF) as u8;

            match (c, x, y, d) {
                (0x0, 0x0, 0x0, 0x0) => return,
                (0x0, 0x0, 0xE, 0xE) => self.ret(),
                (0x0, 0x0, 0xE, 0x0) => self.cls(),
                (0x1, _, _, _) => self.jmp(nnn),
                (0x2, _, _, _) => self.call(nnn),
                (0x3, _, _, _) => self.se(x as usize, kk),
                (0x4, _, _, _) => self.sne(x as usize, kk),
                (0x5, _, _, 0x0) => self.se_xy(x as usize, y as usize),
                (0x6, _, _, _) => self.ld_x(x as usize, kk),
                (0x7, _, _, _) => self.add_x(x as usize, kk),
                (0x8, _, _, 0x0) => self.ld_xy(x as usize, y as usize),
                (0x8, _, _, 0x1) => self.or_xy(x as usize, y as usize),
                (0x8, _, _, 0x2) => self.and_xy(x as usize, y as usize),
                (0x8, _, _, 0x3) => self.xor_xy(x as usize, y as usize),
                (0x8, _, _, 0x4) => self.add_xy(x, y),
                (0x8, _, _, 0x5) => self.sub_xy(x, y),
                (0x8, _, _, 0x6) => self.shr_x(x as usize),
                (0x8, _, _, 0x7) => self.subn_xy(x as usize, y as usize),
                (0x8, _, _, 0xE) => self.shl_x(x as usize),
                (0x9, _, _, 0x0) => self.sne_xy(x as usize, y as usize),
                (0xA, _, _, _) => self.ld_i(nnn),
                (0xB, _, _, _) => self.jmp(self.regs[0] as u16 + nnn),
                (0xC, _, _, _) => self.rnd(x as usize, kk),
                (0xD, _, _, _) => self.drw_xyn(x as usize, y as usize, d),
                (0xE, _, 0x9, 0xE) => self.skp_x(x as usize),
                (0xE, _, 0xA, 0x1) => self.sknp_x(x as usize),
                (0xF, _, 0x0, 0x7) => self.ld_xdt(x as usize),
                (0xF, _, 0x0, 0xA) => self.ld_xk(x as usize),
                (0xF, _, 0x1, 0x5) => self.ld_dtx(x as usize),
                (0xF, _, 0x1, 0x8) => self.ld_st(x as usize),
                (0xF, _, 0x1, 0xE) => self.add_ix(x as usize),
                (0xF, _, 0x2, 0x9) => self.ld_fx(x as usize),
                (0xF, _, 0x3, 0x3) => self.ld_bcd(x as usize),
                (0xF, _, 0x5, 0x5) => self.ld_ix(x as usize),
                (0xF, _, 0x6, 0x5) => self.ld_xi(x as usize),

                _ => todo!("TODO: opcode {:04x}", opcode),
            }
            let clock_hertz = 3_200_000u64;
            let speed = 1_000_000_000 / clock_hertz;
            thread::sleep(time::Duration::from_nanos(speed));
        }
    }

    fn cls(&mut self) {
        for i in 0..self.display.len() {
            for j in 0..self.display[i].len() {
                self.display[j][i] = false;
            }
        }
    }

    fn call(&mut self, addr: u16) {
        self.stack[self.sp] = self.pc as u16;

        if self.sp > self.stack.len() {
            panic!("Stack overflow!");
        }

        self.sp += 1;
        self.pc = addr as usize;
    }

    fn ret(&mut self) {
        if self.sp == 0 {
            panic!("Stack underflow!");
        }

        self.sp -= 1;
        self.pc = self.stack[self.sp] as usize;
    }

    fn jmp(&mut self, addr: u16) {
        self.pc = addr as usize;
    }

    fn se(&mut self, reg: usize, byte: u8) {
        if self.regs[reg] == byte {
            let _ = self.next();
        }
    }

    fn sne(&mut self, reg: usize, byte: u8) {
        if self.regs[reg] != byte {
            let _ = self.next();
        }
    }

    fn se_xy(&mut self, x: usize, y: usize) {
        if self.regs[x] == self.regs[y] {
            let _ = self.next();
        }
    }

    fn ld_x(&mut self, x: usize, byte: u8) {
        self.regs[x] = byte;
    }

    fn add_x(&mut self, x: usize, byte: u8) {
        let (result, _) = self.regs[x].overflowing_add(byte);
        self.regs[x] = result;
    }

    fn ld_xy(&mut self, x: usize, y: usize) {
        self.regs[x] = self.regs[y];
    }

    fn or_xy(&mut self, x: usize, y: usize) {
        self.regs[x] = self.regs[x] | self.regs[y];
    }

    fn and_xy(&mut self, x: usize, y: usize) {
        self.regs[x] = self.regs[x] & self.regs[y];
    }

    fn xor_xy(&mut self, x: usize, y: usize) {
        self.regs[x] = self.regs[x] ^ self.regs[y];
    }

    fn add_xy(&mut self, x: u8, y: u8) {
        let arg1 = self.regs[x as usize];
        let arg2 = self.regs[y as usize];

        let (val, overflow) = arg1.overflowing_add(arg2);
        self.regs[0xF] = overflow as u8;
        self.regs[x as usize] = val;
    }

    fn sub_xy(&mut self, x: u8, y: u8) {
        let arg1 = self.regs[x as usize];
        let arg2 = self.regs[y as usize];

        let (val, overflow) = arg1.overflowing_sub(arg2);
        self.regs[0xF] = !overflow as u8;
        self.regs[x as usize] = val;
    }

    fn shr_x(&mut self, x: usize) {
        self.regs[0xF] = self.regs[x] & 0x01;
        self.regs[x] >>= 1;
    }

    fn subn_xy(&mut self, x: usize, y: usize) {
        let arg1 = self.regs[x as usize];
        let arg2 = self.regs[y as usize];

        let (val, overflow) = arg1.overflowing_sub(arg2);
        self.regs[0xF] = overflow as u8;
        self.regs[x as usize] = val;
    }

    fn shl_x(&mut self, x: usize) {
        self.regs[0xF] = self.regs[x] & (0x01 << 7) >> 7;
        self.regs[x] <<= 1;
    }

    fn sne_xy(&mut self, x: usize, y: usize) {
        if self.regs[x] != self.regs[y] {
            let _ = self.next();
        }
    }

    fn ld_i(&mut self, val: u16) {
        self.index = val;
    }

    fn rnd(&mut self, x: usize, val: u8) {
        let r = rand::random::<u8>();
        self.regs[x] = r & val;
    }

    fn drw_xyn(&mut self, x: usize, y: usize, len: u8) {
        let i = self.index;
        let mut display_index = 0;
        // get a slice from the main memory which represents the sprite to be drawn
        let sprite = &self.memory[i as usize..(i + len as u16) as usize];
        // sprites are 8 pixel wide, so each u8 represents a row
        for (row_idx, sprite_row) in sprite.iter().enumerate() {
            for (col_idx, bit) in sprite_row.to_bits().iter().enumerate() {
                self.display[(self.regs[y] as usize + row_idx) % 32][(self.regs[x] as usize + col_idx) % 64] ^= *bit;
                if self.display[(self.regs[y] as usize + row_idx) % 32][(self.regs[x] as usize + col_idx) % 64] == false && *bit == true {
                    self.regs[0xF] = 1;
                }
            }
        }
        self.draw_display();
    }

    fn skp_x(&mut self, x: usize) {
        let key = self.keyboard[x];
        if key {
            let _ = self.next();
        }
    }

    fn sknp_x(&mut self, x: usize) {
        let key = self.keyboard[x];
        if !key {
            let _ = self.next();
        }
    }

    fn ld_xdt(&mut self, x: usize) {
        self.regs[x] = self.delay_timer;
    }

    fn ld_xk(&mut self, x: usize) {
        thread::sleep(time::Duration::from_secs(1));

        for k in 0..self.keyboard.len() {
            if self.keyboard[k] {
                self.regs[x] = k as u8;
                return;
            }
        }
    }

    fn ld_dtx(&mut self, x: usize) {
        self.delay_timer = self.regs[x];
    }

    fn ld_st(&mut self, x: usize) {
        self.sound_timer = self.regs[x];
    }

    fn add_ix(&mut self, x: usize) {
        self.index += self.regs[x] as u16;
    }

    fn ld_fx(&mut self, x: usize) {
        self.index = self.regs[x] as u16;
    }

    fn ld_bcd(&mut self, x: usize) {
        self.memory[(self.index + 0) as usize] = self.regs[x];
        self.memory[(self.index + 1) as usize] = self.regs[x];
        self.memory[(self.index + 2) as usize] = self.regs[x];
    }

    fn ld_ix(&mut self, x: usize) {
        for i in 0..=x {
            self.memory[(self.index as usize + i)] = self.regs[i];
        }
    }

    fn ld_xi(&mut self, x: usize) {
        for i in 0..=x {
            self.regs[i] = self.memory[(self.index as usize + i)];
        }
    }

    fn draw_display(&mut self) {
        for x in 0..64 {
            for y in 0..32 {
                match self.display[y][x] {
                    true => {
                        print!("{}{}", cursor::Goto((x + 1) as u16, (y + 1) as u16), "█")
                    }
                    false => {
                        print!("{}{}", cursor::Goto((x + 1) as u16, (y + 1) as u16), " ")
                    }
                }
            }
        }
        io::stdout().flush().unwrap();
    }

    fn check_terminal_size(&self) {
        let size = terminal_size().unwrap();

        if size.0 < 64 || size.1 < 32 {
            panic!("Terminal must be at least 64x32 to draw the CHIP-8 Display");
        }
    }
}

fn main() {
    let mut cpu = CPU::new();
    let mem = &mut cpu.memory;
    let mut file = File::open("particle_demo.ch8").unwrap();
    file.read(&mut mem[0x200..]).unwrap();
    print!("{}", termion::cursor::Hide);
    //    cpu.draw_display();
    println!("{}", clear::All);
    cpu.run();
    //   println!("{:x?}", cpu.memory);
}

#[cfg(test)]
#[test]
fn add() {
    let mut cpu = CPU::new();
    let mem = &mut cpu.memory;
    mem[0x200] = 0x80;
    mem[0x201] = 0x14;
    cpu.regs[0] = 5;
    cpu.regs[1] = 10;

    cpu.run();

    assert_eq!(cpu.regs[0], 15);
}

#[test]
fn sub() {
    let mut cpu = CPU::new();
    let mem = &mut cpu.memory;
    mem[0x200] = 0x80;
    mem[0x201] = 0x15;
    cpu.regs[0] = 15;
    cpu.regs[1] = 8;

    cpu.run();

    assert_eq!(cpu.regs[0], 7);
}
