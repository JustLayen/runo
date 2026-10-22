#!/usr/bin/env python3
"""Black-box integration tests using a real Unix pseudo-terminal; stdlib only.
Run after cargo build: python3 tests/pty_smoke.py [path/to/runo]
"""
import errno
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import sys
import tempfile
import termios
import time

BINARY = str(Path(sys.argv[1] if len(sys.argv) > 1 else 'target/debug/runo').resolve())

class Session:
    def __init__(self, *args, cols=80, rows=24, env=None):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
            os.environ['TERM'] = 'xterm-256color'
            if env:
                os.environ.update(env)
            os.execv(BINARY, [BINARY, *map(str, args)])
        self.output = b''
        self.status = None
        self.drain(.12)

    def drain(self, timeout=.06):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            readable, _, _ = select.select([self.fd], [], [], max(0, end-time.monotonic()))
            if not readable:
                break
            try:
                data = os.read(self.fd, 65536)
            except OSError as exc:
                if exc.errno == errno.EIO:
                    break
                raise
            if not data:
                break
            self.output += data
        return self.output

    def send(self, data, wait=.06):
        os.write(self.fd, data.encode() if isinstance(data, str) else data)
        self.drain(wait)
        return self

    def finish(self):
        end = time.monotonic()+3
        while time.monotonic() < end:
            pid, status = os.waitpid(self.pid, os.WNOHANG)
            if pid:
                self.status = status
                os.close(self.fd)
                assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0, self.output[-2000:]
                return
            self.drain(.05)
        self.kill()
        raise AssertionError('Editor did not exit: '+repr(self.output[-3000:]))

    def kill(self):
        try:
            os.kill(self.pid, signal.SIGKILL)
            os.waitpid(self.pid, 0)
        except ProcessLookupError:
            pass
        try:
            os.close(self.fd)
        except OSError:
            pass


def check():
    with tempfile.TemporaryDirectory(prefix='runo-pty-') as d:
        d=Path(d)
        p=d/'basic.txt'
        s=Session('-I',p)
        assert b'MIT/Runo 0.1' in s.output
        s.send('Привет 世界\rsecond').send(b'\x13').send(b'\x18').finish()
        assert p.read_bytes()=='Привет 世界\nsecond\n'.encode(),p.read_bytes()
        print('PASS Unicode input, save, exit, branding')

        p=d/'undo.txt'
        s=Session('-I',p)
        s.send('abc').send(b'\x1bu').send('xyz').send(b'\x13').send('q').send(b'\x1bu').send(b'\x18').finish()
        assert p.read_text()=='xyz\n',p.read_text()
        print('PASS undo branch and edit after save')

        p=d/'cut.txt'
        p.write_text('first\nsecond\nthird\n')
        s=Session('-I',p)
        s.send(b'\x0b').send(b'\x0b').send(b'\x15').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='first\nsecond\nthird\n',p.read_text()
        print('PASS consecutive line cuts and multiline paste')

        p=d/'replace.txt'
        p.write_text('world WORLD\nworld\n')
        s=Session('-I',p)
        s.send(b'\x1c').send('world\r').send('Rust\r').send('a').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='Rust Rust\nRust\n',p.read_text()
        print('PASS interactive case-insensitive replace all')

        p=d/'insert.txt'
        source=d/'source.txt'
        source.write_text('read\nfile\n')
        s=Session('-I',p)
        s.send(b'\x12').send(str(source)+'\r').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='read\nfile\n',p.read_text()
        print('PASS read file dialog')

        p=d/'command.txt'
        s=Session('-I',p)
        s.send(b'\x14').send("printf 'command\\noutput\\n'\r",.2).send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='command\noutput\n',p.read_text()
        print('PASS execute command and insert output')

        p=d/'pipe.txt'
        p.write_text('hello\n')
        s=Session('-I',p)
        s.send(b'\x14').send("| tr a-z A-Z\r",.2).send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='HELLO\n',p.read_text()
        print('PASS pipe buffer through command')

        p=d/'dos.txt'
        p.write_bytes(b'a\r\nb\r\n')
        s=Session('-I','-N',p)
        s.send('X').send(b'\x13').send(b'\x18').finish()
        assert p.read_bytes()==b'Xa\r\nb\r\n',p.read_bytes()
        print('PASS --noconvert preserves CRLF without doubling CR')

        p=d/'no-newline.txt'
        s=Session('-IL',p)
        s.send('abc').send(b'\x13').send(b'\x18').finish()
        assert p.read_bytes()==b'abc'
        print('PASS --nonewlines')

        p=d/'backup.txt'
        p.write_text('old\n')
        s=Session('-IB',p)
        s.send('new ').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='new old\n'
        assert p.with_name(p.name+'~').read_text()=='old\n'
        print('PASS backups')

        p=d/'help.txt'
        s=Session('-I',p)
        s.send(b'\x07').send(b'\x18').send('after help').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='after help\n'
        print('PASS help opens and returns to original buffer')

        p=d/'small.txt'
        s=Session('-I',p,cols=8,rows=4)
        s.send('世界\tabc\r').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='世界\tabc\n\n',repr(p.read_text())
        print('PASS narrow terminal')


        p=d/'macro.txt'
        s=Session('-I',p)
        s.send(b'\x14').send(b'\x1b:').send('abc').send(b'\x14').send(b'\x1b:').send(b'\x1b:').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='abcabc\n',p.read_text()
        print('PASS macro recording and playback')

        p=d/'selected.txt'
        p.write_text('abcd\n')
        s=Session('-I',p)
        s.send(b'\x1ba').send(b'\x1b[C\x1b[C').send(b'\x0b').send(b'\x15').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='abcd\n',p.read_text()
        print('PASS marked region cut and paste')

        p=d/'paste.txt'
        s=Session('-I',p)
        s.send(b'\x1b[200~one\ntwo\n\x1b[201~').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='one\ntwo\n',p.read_text()
        print('PASS bracketed multiline paste')

        p=d/'goto.txt'
        p.write_text('one\ntwo\nthree\n')
        s=Session('-I',p)
        s.send(b'\x1bg').send('2,2\r').send('X').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='one\ntXwo\nthree\n',p.read_text()
        print('PASS go to line and column')

        p=d/'rc.txt'
        rc=d/'runorc'
        rc.write_text('set tabsize 3\nset tabstospaces\nbind M-Q "{savefile}{exit}" main\n')
        s=Session('-f',rc,p)
        s.send('a\tb').send(b'\x1bq').finish()
        assert p.read_text()=='a  b\n',p.read_text()
        print('PASS rc settings and bound command expansion')

        p=d/'locked.txt'
        p.write_text('hello\n')
        s=Session('-IG',p)
        lock=p.with_name('.'+p.name+'.swp')
        assert lock.read_bytes().startswith(b'b0runo 0.1')
        s.send(b'\x18').finish()
        assert not lock.exists()
        print('PASS advisory lock creation and cleanup')

        p=d/'changed.txt'
        p.write_text('original\n')
        s=Session('-I',p)
        s.send('my ')
        p.write_text('external\n')
        s.send(b'\x13').send('n').send(b'\x18').send('n').finish()
        assert p.read_text()=='external\n'
        print('PASS external modification overwrite protection')


        p=d/'softwrap.txt'
        s=Session('-IS',p,cols=12,rows=8)
        s.send('abcdefghijklmnopqrst').send(b'\x1b[D\x1b[D').send('X').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='abcdefghijklmnopqrXst\n',p.read_text()
        print('PASS left movement within a wrapped line')

        p=d/'wrapped-page.txt'
        original='abcdefghijklmnopqrstuvwxyz'*5+'\n'
        p.write_text(original)
        s=Session('-IS',p,cols=12,rows=8)
        s.send(b'\x1b[6~').send('X').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()==original[:24]+'X'+original[24:],p.read_text()
        print('PASS page movement within a wrapped logical line')

        directory=d/'browser'
        directory.mkdir()
        source=directory/'target.txt'
        source.write_text('from browser\n')
        p=d/'browser-result.txt'
        s=Session('-I',p)
        s.send(b'\x12').send(str(directory)+'/').send(b'\x14').send(b'\x1b[F').send('\r').send('\r').send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='from browser\n',p.read_text()
        print('PASS file browser selection and insertion')

        p=d/'first-buffer.txt'
        other=d/'second-buffer.txt'
        p.write_text('first\n')
        other.write_text('second\n')
        s=Session('-I',p,other)
        s.send('A').send(b'\x13').send(b'\x1b>').send('B').send(b'\x13').send(b'\x18').send(b'\x18').finish()
        assert p.read_text()=='Afirst\n'
        assert other.read_text()=='Bsecond\n'
        print('PASS multiple buffers and switching')

        p=d/'cancel-command.txt'
        s=Session('-I',p)
        s.send('safe').send(b'\x14').send('sleep 30 & wait\r',.2).send(b'\x03',.2).send(b'\x13').send(b'\x18').finish()
        assert p.read_text()=='safe\n'
        print('PASS cancelling a command preserves buffer')

        p=d/'signal.txt'
        s=Session('-I',p)
        s.send('unsaved')
        os.kill(s.pid, signal.SIGTERM)
        s.finish()
        assert p.with_name(p.name+'.save').read_text()=='unsaved\n'
        print('PASS emergency save on SIGTERM')

if __name__=='__main__':
    check()
