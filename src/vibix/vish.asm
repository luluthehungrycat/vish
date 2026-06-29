;==============================================================================
; vish.asm — VIBIX SHell (flat binary for bare-metal VIBIX)
;
; A standalone interactive shell for the VIBIX kernel, running as a userspace
; flat binary. Provides readline, command history, and built-in commands.
;
; Build:  nasm -f bin src/vibix/vish.asm -o vish.bin
;
; VIBIX syscall ABI:
;   rax = syscall#, rdi/rsi/rdx/r8/r9 = args, return in rax
;   ALL registers clobbered except rcx, r11 (preserved by CPU)
;
; Syscall numbers:
;   0 = exit(int code)
;   1 = write(int fd, const void *buf, size_t len)
;   2 = read(int fd, void *buf, size_t len)
;   3 = getpid(void)
;   4 = brk(void *addr)
;   8 = fork(void)
;   9 = exec(path, argv, envp)
;  10 = waitpid(pid, *wstatus, flags)
;  12 = open(path, flags)
;  13 = close(fd)
;==============================================================================

; ── Configuration ──────────────────────────────────────────────────────────────
%define PROMPT_TEXT   "vish$ "
LINE_CAPACITY   equ 256       ; max characters per input line
ARGV_CAPACITY   equ 16        ; max arguments per command
HISTORY_SLOTS   equ 4         ; remembered commands
HISTORY_STRIDE  equ LINE_CAPACITY
READ_CHUNK      equ 256       ; buffer size for cat and I/O

; ── Entry point ────────────────────────────────────────────────────────────────
ORG 0x2000000
bits 64
section .text
global _start

_start:
    ; VIBIX loads flat binary at 0x2000000. Stack is in its own page.
    mov rsp, 0x2003000                     ; top of 4 KiB stack page (USER_STACK_ADDR + 0x1000)

    ; Save environment pointer (passed by VIBIT or kernel ELF loader)
    ; rdx = envp (null-terminated array of "NAME=value" strings)
    mov r15, rdx                           ; r15 = envp (preserved throughout)

    ; ── Initialize shell state ──────────────────────────────────────────────
    call shell_init

; ── Main read-eval-print loop ──────────────────────────────────────────────────
repl_loop:
    ; Print prompt
    lea rsi, [rel str_prompt]
    mov rdi, 1
    call write_string

    ; Read one line of input
    lea rdi, [rel line_buffer]
    mov rsi, LINE_CAPACITY
    call read_line

    ; Check for EOF (no chars read)
    cmp rax, 0
    je repl_exit

    ; Record in history (line in rdi, length in rax)
    push rax
    lea rdi, [rel line_buffer]
    mov rsi, rax
    call record_history
    pop rax

    ; Tokenize the line
    lea rdi, [rel line_buffer]             ; input string
    mov rsi, rax                           ; length
    lea rdx, [rel argv_table]              ; output argv array
    mov rcx, ARGV_CAPACITY                ; max args
    call tokenize_line

    ; rax = argc, argv_table filled
    cmp rax, 0
    je repl_loop                           ; skip empty lines

    ; Look up and dispatch command
    lea rdi, [rel argv_table]
    mov rsi, rax                           ; argc
    lea rdx, [rel command_table]
    call dispatch_command

    ; Check exit flag
    cmp byte [rel exit_flag], 1
    je repl_exit

    jmp repl_loop

repl_exit:
    lea rsi, [rel str_farewell]
    mov rdi, 1
    call write_string

    mov rax, 0
    call sys_exit

; ── shell_init ──────────────────────────────────────────────────────────────────
; Initialize shell state: clear history buffer, print welcome.
shell_init:
    push rcx
    push rdi
    push rax

    ; Zero-fill history buffer
    lea rdi, [rel history_buffer]
    mov rcx, HISTORY_SLOTS * HISTORY_STRIDE
    xor al, al
    rep stosb

    ; Clear exit flag
    mov byte [rel exit_flag], 0

    ; Clear history index
    mov qword [rel history_count], 0
    mov qword [rel history_index], 0

    ; Save environment pointer (r15 = envp from _start)
    mov [rel saved_envp], r15

    ; Print welcome banner
    lea rsi, [rel str_welcome]
    mov rdi, 1
    call write_string

    pop rax
    pop rdi
    pop rcx
    ret

; ── read_line ───────────────────────────────────────────────────────────────────
; Read a line from stdin character-by-character.
; Arguments:
;   rdi = buffer address
;   rsi = buffer capacity
; Returns:
;   rax = number of characters read (0 = EOF)
read_line:
    push rbx
    push rcx
    push rdx
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8

    mov r12, rdi                           ; r12 = buffer
    mov r13, rsi                           ; r13 = capacity
    xor r14, r14                           ; r14 = index (chars read)

.read_char:
    ; Check for buffer full
    cmp r14, r13
    jae .done

    ; Read one byte from stdin
    lea rdi, [rel char_buf]
    mov rsi, 1
    call sys_read
    cmp rax, 1
    je .got_char
    test rax, rax
    js .check_eof                           ; rax < 0: error, treat as EOF
    jmp .read_char                          ; rax = 0: no data yet, retry

.got_char:
    movzx r15, byte [rel char_buf]         ; r15 = input char

    ; ── Handle control characters ───────────────────────────────────────────

    cmp r15, 0x1b                          ; ESC
    je .handle_esc

    cmp r15, 0x7f                          ; DEL (backspace)
    je .handle_backspace
    cmp r15, 0x08                          ; BS (backspace alternative)
    je .handle_backspace

    cmp r15, 0x04                          ; Ctrl+D
    je .handle_ctrld

    cmp r15, 0x03                          ; Ctrl+C
    je .handle_ctrlc

    cmp r15, 0x0a                          ; LF
    je .handle_enter
    cmp r15, 0x0d                          ; CR
    je .handle_enter

    ; ── Printable character ──────────────────────────────────────────────────
    cmp r15, 0x20
    jb .read_char                          ; skip other control chars

    mov [r12 + r14], r15b                  ; store char
    inc r14

    ; Echo back
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write

    jmp .read_char

.handle_backspace:
    cmp r14, 0
    je .read_char                          ; nothing to erase

    dec r14                                ; remove from buffer

    ; Backspace sequence: BS + space + BS
    push r14
    mov byte [rel char_buf], 0x08
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], 0x20
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], 0x08
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    pop r14

    jmp .read_char

.handle_enter:
    mov byte [r12 + r14], 0                ; null-terminate
    ; Write CRLF
    mov byte [rel char_buf], 0x0d
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], 0x0a
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    jmp .done

.handle_ctrld:
    cmp r14, 0
    jne .read_char                         ; only EOF on empty line

    xor r14, r14                           ; return 0 (EOF)
    jmp .done

.handle_ctrlc:
    ; Clear the line and restart
    mov r14, 0
    ; Write ^C then newline
    mov byte [rel char_buf], '^'
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], 'C'
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], 0x0d
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], 0x0a
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    ; Re-display prompt and continue reading
    lea rsi, [rel str_prompt]
    mov rdi, 1
    call write_string
    jmp .read_char

.handle_esc:
    ; Read next two bytes for arrow keys
    lea rdi, [rel char_buf]
    mov rsi, 1
    call sys_read
    cmp rax, 1
    jne .read_char

    movzx r15, byte [rel char_buf]        ; second byte
    cmp r15, 0x5b                         ; '['
    jne .read_char

    lea rdi, [rel char_buf]
    mov rsi, 1
    call sys_read
    cmp rax, 1
    jne .read_char

    movzx r15, byte [rel char_buf]        ; third byte

    cmp r15, 0x41                         ; Up arrow
    je .history_up
    cmp r15, 0x42                         ; Down arrow
    je .history_down

    jmp .read_char                         ; other ESC sequences ignored

.history_up:
    ; Navigate to previous history entry
    lea rdi, [rel history_buffer]
    lea rsi, [rel line_buffer]
    call history_navigate_up
    cmp rax, 0
    je .read_char                          ; no history

    ; Replace current buffer with history entry
    mov r14, rax                           ; new length
    ; Redraw the line: CR, clear to end of line, prompt, new content
    push r14
    call redraw_line
    pop r14
    jmp .read_char

.history_down:
    ; Navigate to next history entry (or back to current input)
    lea rdi, [rel history_buffer]
    lea rsi, [rel line_buffer]
    call history_navigate_down
    ; rax = 0 means back to empty
    mov r14, rax
    call redraw_line
    jmp .read_char

.check_eof:
    ; read returned 0 or < 0 (no more input)
    xor r14, r14

.done:
    mov rax, r14
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rdx
    pop rcx
    pop rbx
    ret

; ── redraw_line ────────────────────────────────────────────────────────────────
; Redraw the current input line after history navigation.
; Arguments:
;   r14 = buffer length
; Uses char_buf as scratch
redraw_line:
    push rax
    push rcx
    push rdi
    push rsi

    ; CR
    mov byte [rel char_buf], 0x0d
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write

    ; Clear to end of line (ESC [ K)
    mov byte [rel char_buf], 0x1b
    mov byte [rel char_buf+1], 0x5b
    mov byte [rel char_buf+2], 0x4b
    lea rdi, [rel char_buf]
    mov rsi, 3
    mov rdi, 1
    call sys_write

    ; Print prompt
    lea rsi, [rel str_prompt]
    mov rdi, 1
    call write_string

    ; Print buffer content
    cmp r14, 0
    je .done_redraw
    lea rsi, [rel line_buffer]
    mov rdi, 1
    mov rdx, r14
    call sys_write

.done_redraw:
    pop rsi
    pop rdi
    pop rcx
    pop rax
    ret

; ── history_navigate_up ─────────────────────────────────────────────────────────
; Browse to previous history entry.
; Arguments:
;   rdi = history buffer base
;   rsi = line buffer (to save current input)
; Returns:
;   rax = length of history entry (0 = no more history)
history_navigate_up:
    push rbx
    push rcx
    push rdx
    push rdi
    push rsi

    mov rbx, rdi                           ; rbx = history base
    mov rcx, rsi                           ; rcx = line buffer

    ; Load current browse index
    mov rax, qword [rel history_browse]

    cmp rax, 0
    jne .already_browsing

    ; First up-arrow: save current line as pending
    ; (In the current implementation, we don't save pending input —
    ;  we just browse. Could add this later.)

.already_browsing:
    ; Compare browse index with history count
    mov rdx, qword [rel history_count]
    cmp rax, rdx
    jae .no_more_history                   ; browse index >= count

    ; Load history entry at position (count - 1 - browse)
    mov rdi, rax                           ; rdi = browse index
    inc rax
    mov qword [rel history_browse], rax    ; increment browse index

    ; Calculate offset: (count - browse) * HISTORY_STRIDE
    mov rax, rdx                           ; rax = count
    sub rax, rdi                           ; rax = count - (old browse)
    dec rax                                ; adjust for inc above
    imul rax, HISTORY_STRIDE

    ; Copy history entry to line buffer
    lea rsi, [rbx + rax]                   ; source in history
    mov rdi, rcx                           ; dest = line buffer
    mov rcx, LINE_CAPACITY
    call string_copy

    ; Write null terminator
    mov byte [rdi], 0

    ; Return length
    mov rdi, rcx
    call string_length
    jmp .done_up

.no_more_history:
    xor rax, rax

.done_up:
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rbx
    ret

; ── history_navigate_down ───────────────────────────────────────────────────────
; Browse to next (newer) history entry.
; Arguments:
;   rdi = history buffer base
;   rsi = line buffer
; Returns:
;   rax = length of entry (0 = back to current/pending)
history_navigate_down:
    push rbx
    push rcx
    push rdx
    push rdi
    push rsi

    mov rbx, rdi                           ; rbx = history base

    ; Load browse index
    mov rax, qword [rel history_browse]
    cmp rax, 1
    jb .bottom_reached                     ; browse = 0 or unset

    ; Decrement browse index
    dec rax
    mov qword [rel history_browse], rax

    ; Calculate offset: (count - browse) * HISTORY_STRIDE
    mov rdx, qword [rel history_count]
    mov rdi, rax                           ; rdi = new browse
    mov rax, rdx                           ; rax = count
    sub rax, rdi
    dec rax
    imul rax, HISTORY_STRIDE

    ; Copy to line buffer
    lea rsi, [rbx + rax]
    mov rdi, rcx
    mov rcx, LINE_CAPACITY
    call string_copy
    mov byte [rdi], 0

    ; Return length
    mov rdi, rcx
    call string_length
    jmp .done_down

.bottom_reached:
    ; Clear browse and return empty
    mov qword [rel history_browse], 0
    ; Clear line buffer
    mov byte [rcx], 0
    xor rax, rax

.done_down:
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rbx
    ret

; ── record_history ──────────────────────────────────────────────────────────────
; Record a command in the history ring buffer.
; Arguments:
;   rdi = pointer to command string
;   rsi = length of command
record_history:
    push rbx
    push rcx
    push rdx
    push rdi
    push rsi

    ; Ignore empty lines
    cmp rsi, 0
    je .skip_history

    ; Dedup against last entry
    mov rcx, qword [rel history_count]
    cmp rcx, 0
    je .do_record

    ; Compare with most recent entry
    lea rbx, [rel history_buffer]
    mov rax, rcx
    dec rax
    imul rax, HISTORY_STRIDE
    lea rbx, [rbx + rax]                   ; rbx = last entry

    push rdi
    push rsi
    mov rdi, rsi                           ; current command
    mov rsi, rbx                           ; last entry
    call string_equal
    pop rsi
    pop rdi
    cmp rax, 1
    je .skip_history                       ; same as last, skip

.do_record:
    ; If history is full, shift entries up
    mov rcx, qword [rel history_count]
    cmp rcx, HISTORY_SLOTS
    jb .append_history                     ; still have room

    ; Shift all entries up by one (drop oldest)
    push rsi
    push rdi
    mov rcx, (HISTORY_SLOTS - 1) * HISTORY_STRIDE
    lea rdi, [rel history_buffer]
    lea rsi, [rel history_buffer + HISTORY_STRIDE]
    rep movsb
    pop rdi
    pop rsi
    mov rcx, HISTORY_SLOTS - 1
    jmp .write_slot

.append_history:
    ; Append at count position
    mov rcx, qword [rel history_count]

.write_slot:
    ; Copy command into slot rcx
    push rcx
    imul rcx, HISTORY_STRIDE
    lea rdi, [rel history_buffer + rcx]
    mov rcx, rsi
    cmp rcx, LINE_CAPACITY - 1
    jb .copy_ok
    mov rcx, LINE_CAPACITY - 1
.copy_ok:
    rep movsb
    mov byte [rdi], 0                       ; null-terminate
    pop rcx

    ; Increment count (cap at HISTORY_SLOTS)
    mov rax, qword [rel history_count]
    inc rax
    cmp rax, HISTORY_SLOTS
    jbe .set_count
    mov rax, HISTORY_SLOTS
.set_count:
    mov qword [rel history_count], rax

.skip_history:
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rbx
    ret

; ── tokenize_line ───────────────────────────────────────────────────────────────
; Split a line into argv array (in-place, modifies input).
; Handles: whitespace splitting, single/double quotes.
; Arguments:
;   rdi = input string (null-terminated)
;   rsi = length
;   rdx = output argv array (array of pointers)
;   rcx = max args
; Returns:
;   rax = argc (number of tokens found)
tokenize_line:
    push rbx
    push r12
    push r13
    push r14
    push r15

    mov r12, rdi                           ; r12 = input
    mov r13, rdx                           ; r13 = argv array
    mov r14, rcx                           ; r14 = max args
    xor r15, r15                           ; r15 = argc

    ; Ensure null-terminated (input already is)

.next_token:
    cmp r15, r14
    jae .done_tokenize

    ; Skip whitespace
.skip_ws:
    movzx rax, byte [r12]
    cmp al, 0
    je .done_tokenize
    cmp al, ' '
    je .skip_and_advance
    cmp al, 0x09                           ; tab
    je .skip_and_advance
    jmp .start_token

.skip_and_advance:
    inc r12
    jmp .skip_ws

.start_token:
    ; Save pointer to start of token
    mov [r13 + r15*8], r12
    inc r15

    ; Scan to end of token
.scan_token:
    movzx rax, byte [r12]

    cmp al, 0
    je .done_tokenize
    cmp al, ' '
    je .end_token
    cmp al, 0x09
    je .end_token

    cmp al, 0x27
    je .handle_single_quote
    cmp al, '"'
    je .handle_double_quote

    inc r12
    jmp .scan_token

.handle_single_quote:
    ; Skip opening quote, scan until closing quote
    inc r12                                 ; move past '
    ; Shift remaining token left by 1 to remove the opening '
    ; For simplicity, we just scan and null-check
.scan_sq:
    movzx rax, byte [r12]
    cmp al, 0
    je .done_tokenize                       ; unterminated — stop
    cmp al, 0x27
    je .end_sq
    inc r12
    jmp .scan_sq
.end_sq:
    inc r12                                 ; move past closing '
    jmp .scan_token

.handle_double_quote:
    inc r12                                 ; move past "
.scan_dq:
    movzx rax, byte [r12]
    cmp al, 0
    je .done_tokenize
    cmp al, '"'
    je .end_dq
    cmp al, 0x5c
    je .skip_dq_escape
    inc r12
    jmp .scan_dq
.skip_dq_escape:
    inc r12                                 ; skip backslash
    movzx rax, byte [r12]
    cmp al, 0
    je .done_tokenize
    inc r12
    jmp .scan_dq
.end_dq:
    inc r12                                 ; move past closing "
    jmp .scan_token

.end_token:
    ; Null-terminate the token
    mov byte [r12], 0
    inc r12
    jmp .next_token

.done_tokenize:
    mov rax, r15

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; ── dispatch_command ───────────────────────────────────────────────────────────
; Look up command in table and call its handler.
; Arguments:
;   rdi = argv array (array of string pointers)
;   rsi = argc
;   rdx = command table
dispatch_command:
    push rbx
    push r12
    push r13
    push r14
    push r15

    mov r12, rdi                           ; r12 = argv
    mov r13, rsi                           ; r13 = argc
    mov r14, rdx                           ; r14 = table

    ; Safety: need at least argv[0]
    cmp r13, 0
    je .not_found

    ; Get command name
    mov rbx, [r12]                         ; rbx = argv[0] (name)

    ; Check direct-dispatch commands first (no table lookup needed)
    lea rdi, [rel cmd_exit]
    mov rsi, rbx
    call string_equal
    cmp rax, 1
    je .do_exit

    lea rdi, [rel cmd_help]
    mov rsi, rbx
    call string_equal
    cmp rax, 1
    je .do_help

    lea rdi, [rel cmd_history_str]
    mov rsi, rbx
    call string_equal
    cmp rax, 1
    je .do_history

    lea rdi, [rel cmd_clear]
    mov rsi, rbx
    call string_equal
    cmp rax, 1
    je .do_clear

    ; Linear search through command table
    mov r15, r14                           ; r15 = table pointer
.search_loop:
    mov rax, [r15]                         ; table entry name pointer
    test rax, rax
    jz .not_found                          ; end of table

    mov rdi, rax
    mov rsi, rbx
    push r15
    call string_equal
    pop r15
    cmp rax, 1
    je .found

    add r15, 16                            ; next entry (pointer + handler)
    jmp .search_loop

.found:
    ; Call the handler: handler(argc, argv)
    mov rax, [r15 + 8]                     ; handler pointer
    mov rdi, r13                           ; argc
    mov rsi, r12                           ; argv
    call rax
    jmp .done_dispatch

.do_exit:
    mov rdi, r13
    mov rsi, r12
    call builtin_exit
    jmp .done_dispatch

.do_help:
    mov rdi, r13
    mov rsi, r12
    call builtin_help
    jmp .done_dispatch

.do_history:
    mov rdi, r13
    mov rsi, r12
    call builtin_history
    jmp .done_dispatch

.do_clear:
    mov rdi, r13
    mov rsi, r12
    call builtin_clear
    jmp .done_dispatch

.not_found:
    ; Try external command via fork/exec
    ; r12 = argv, r15 = envp
    mov rdi, [r12]                         ; cmd_name = argv[0]
    mov rsi, r12                           ; argv
    mov rdx, r15                           ; envp
    call try_find_and_exec
    cmp rax, 0
    je .done_dispatch                      ; external command succeeded

    ; Not found by any method — print error
    mov rsi, [r12]                         ; command name
    mov rdi, 1
    call write_string
    lea rsi, [rel str_not_found]
    mov rdi, 1
    call write_string
    call write_newline

.done_dispatch:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; ── Built-ins ──────────────────────────────────────────────────────────────────

; ── builtin_exit ────────────────────────────────────────────────────────────────
builtin_exit:
    push rbx
    mov rbx, rdi                           ; rbx = argc

    mov byte [rel exit_flag], 1

    cmp rbx, 2
    jb .exit_default_code
    cmp rbx, 2
    ja .exit_usage

    ; Parse exit code from argv[1]
    mov rsi, [rsi + 8]                     ; argv[1]
    call parse_decimal
    jmp .exit_done

.exit_default_code:
    xor rax, rax
    jmp .exit_done

.exit_usage:
    lea rsi, [rel str_exit_usage]
    mov rdi, 1
    call write_string
    call write_newline
    mov byte [rel exit_flag], 0            ; cancel exit
    xor rax, rax
    jmp .exit_done

.exit_done:
    mov qword [rel exit_code_storage], rax
    pop rbx
    ret

; ── builtin_echo ────────────────────────────────────────────────────────────────
builtin_echo:
    push rbx
    push r12
    push r13
    push r14

    mov r12, rdi                           ; r12 = argc
    mov r13, rsi                           ; r13 = argv
    mov r14, 1                             ; r14 = first arg index (skip name)

    ; Parse flags
    mov rbx, 0                             ; no_newline = 0
    mov rcx, 0                             ; parse_escapes = 0

.flag_loop:
    cmp r14, r12
    jae .print_loop

    mov rsi, [r13 + r14*8]                 ; argv[r14]
    movzx rax, byte [rsi]
    cmp al, '-'
    jne .print_loop

    ; Check for -- terminator
    inc rsi
    movzx rax, byte [rsi]
    cmp al, '-'                            ; "--"
    je .flag_done

    ; Parse flag characters
    mov rdi, rsi
.parse_flags:
    movzx rax, byte [rdi]
    test al, al
    jz .flag_consumed
    cmp al, 'n'
    je .flag_n
    cmp al, 'e'
    je .flag_e
    cmp al, 'E'
    je .flag_E
    jmp .print_loop                        ; not a valid flag

.flag_n:
    mov rbx, 1
    inc rdi
    jmp .parse_flags

.flag_e:
    mov rcx, 1
    inc rdi
    jmp .parse_flags

.flag_E:
    mov rcx, 0
    inc rdi
    jmp .parse_flags

.flag_consumed:
    inc r14
    jmp .flag_loop

.flag_done:
    inc r14                                ; skip "--"
    jmp .print_loop

.print_loop:
    cmp r14, r12
    jae .print_done

    ; Separator
    cmp r14, 1
    je .first_arg
    mov byte [rel char_buf], ' '
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
.first_arg:

    ; Write argument (possibly with escape decoding)
    mov rsi, [r13 + r14*8]
    cmp rcx, 1
    je .print_escaped
    mov rdi, 1
    call write_string
    jmp .next_print_arg

.print_escaped:
    mov rdi, rsi
    call write_escaped

.next_print_arg:
    inc r14
    jmp .print_loop

.print_done:
    cmp rbx, 1
    je .echo_done

    mov byte [rel char_buf], 0x0d
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], 0x0a
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write

.echo_done:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; ── builtin_cat ─────────────────────────────────────────────────────────────────
builtin_cat:
    push rbx
    push r12
    push r13
    push r14
    push r15

    mov r12, rdi                           ; argc
    mov r13, rsi                           ; argv
    mov r14, 1                             ; arg index

    cmp r12, 1
    jb .cat_done
    je .cat_stdin                          ; no args → stdin

.cat_file_loop:
    cmp r14, r12
    jae .cat_done

    mov r15, [r13 + r14*8]                 ; argv[r14]
    cmp byte [r15], '-'
    jne .cat_read_file
    cmp byte [r15+1], 0
    je .cat_stdin                           ; "-" means stdin

.cat_read_file:
    ; We'd need a filesystem open call. For now, print error.
    lea rsi, [rel str_cat_notimpl]
    mov rdi, 1
    call write_string
    call write_newline

.inc_cat:
    inc r14
    jmp .cat_file_loop

.cat_stdin:
    ; Read from stdin in chunks and echo to stdout
.cat_stdin_loop:
    lea rdi, [rel read_buffer]
    mov rsi, READ_CHUNK
    call sys_read
    cmp rax, 0
    je .cat_done
    je .cat_stdin_done

    lea rsi, [rel read_buffer]
    mov rdx, rax
    mov rdi, 1
    call sys_write
    jmp .cat_stdin_loop

.cat_stdin_done:
    jmp .cat_done

.cat_done:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; ── builtin_printenv ────────────────────────────────────────────────────────────
builtin_printenv:
    push rbx
    push r12
    push r13
    push r14
    push r15

    mov r12, rdi                           ; argc
    mov r13, rsi                           ; argv
    mov r14, 1                             ; arg index (0 = "printenv")

    cmp r12, 1
    je .printenv_all

.printenv_some:
    cmp r14, r12
    jae .printenv_done

    mov r15, [r13 + r14*8]                 ; variable name

    ; Walk envp looking for "NAME=" or prefix matching
    mov rbx, r15                           ; save name pointer
    mov rsi, r15

    push r12
    push r13
    push r14

    mov r12, r15                           ; name to find
    mov r13, r15                           ; save name
    call string_length
    mov r14, rax                           ; name length

    mov r15, [rel saved_envp]             ; r15 = envp
    test r15, r15
    jz .penv_not_found

.penv_loop:
    mov rsi, [r15]
    test rsi, rsi
    jz .penv_not_found

    ; Check if starts with name=
    mov rdi, r12
    push rsi
    push rdi
    ; compare first name_len chars
    mov rcx, r14
    repe cmpsb
    pop rdi
    pop rsi
    jne .penv_next

    ; Check that next char is '='
    cmp byte [rsi + r14], '='
    jne .penv_next

    ; Found! Print value
    lea rdi, [rsi + r14 + 1]
    mov rdi, 1
    call write_string
    call write_newline
    jmp .penv_done_var

.penv_next:
    add r15, 8
    jmp .penv_loop

.penv_not_found:
.penv_done_var:
    pop r14
    pop r13
    pop r12

    inc r14
    jmp .printenv_some

.printenv_all:
    ; Print all environment variables
    mov r15, [rel saved_envp]
    test r15, r15
    jz .printenv_done

.penv_all_loop:
    mov rsi, [r15]
    test rsi, rsi
    jz .printenv_done

    mov rdi, 1
    call write_string
    call write_newline

    add r15, 8
    jmp .penv_all_loop

.printenv_done:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; ── builtin_clear ───────────────────────────────────────────────────────────────
builtin_clear:
    push rsi

    ; ESC [ 2 J (clear entire screen)
    mov byte [rel char_buf], 0x1b
    mov byte [rel char_buf+1], 0x5b
    mov byte [rel char_buf+2], 0x32
    mov byte [rel char_buf+3], 0x4a
    lea rdi, [rel char_buf]
    mov rsi, 4
    mov rdi, 1
    call sys_write

    ; ESC [ H (cursor home)
    mov byte [rel char_buf], 0x1b
    mov byte [rel char_buf+1], 0x5b
    mov byte [rel char_buf+2], 0x48
    lea rdi, [rel char_buf]
    mov rsi, 3
    mov rdi, 1
    call sys_write

    pop rsi
    ret

; ── builtin_true ────────────────────────────────────────────────────────────────
builtin_true:
    xor rax, rax
    ret

; ── builtin_false ───────────────────────────────────────────────────────────────
builtin_false:
    mov rax, 1
    ret

; ── builtin_help ────────────────────────────────────────────────────────────────
builtin_help:
    push rsi
    lea rsi, [rel help_text]
    mov rdi, 1
    call write_string
    pop rsi
    ret

; ── builtin_history ─────────────────────────────────────────────────────────────
builtin_history:
    push rbx
    push rcx
    push r12
    push r13

    mov r12, qword [rel history_count]
    cmp r12, 0
    je .hist_empty

    xor r13, r13                           ; entry index

.hist_loop:
    cmp r13, r12
    jae .hist_done

    ; Print "  N  command"
    mov byte [rel char_buf], ' '
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], ' '
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write

    ; Print entry number
    mov rdi, r13
    inc rdi                                ; 1-based
    call print_decimal

    ; Print "  "
    mov byte [rel char_buf], ' '
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], ' '
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write

    ; Print command
    mov rax, r13
    imul rax, HISTORY_STRIDE
    lea rsi, [rel history_buffer + rax]
    mov rdi, 1
    call write_string

    call write_newline

    inc r13
    jmp .hist_loop

.hist_empty:
    lea rsi, [rel str_no_history]
    mov rdi, 1
    call write_string
    call write_newline

.hist_done:
    pop r13
    pop r12
    pop rcx
    pop rbx
    ret

; ── String utilities ───────────────────────────────────────────────────────────

; ── write_string ────────────────────────────────────────────────────────────────
; Write a null-terminated string to stdout.
; Arguments:
;   rsi = string pointer
;   rdi = fd (1 for stdout)
write_string:
    push rdx
    push rsi
    push rdi

    mov rdi, rsi
    call string_length
    mov rdx, rax

    pop rdi
    pop rsi
    ; rsi = string, rdx = length, rdi = fd
    call sys_write

    pop rdx
    ret

; ── write_newline ───────────────────────────────────────────────────────────────
write_newline:
    push rsi
    push rdi
    mov byte [rel char_buf], 0x0d
    lea rsi, [rel char_buf]
    mov rdi, 1
    mov rdx, 1
    call sys_write
    mov byte [rel char_buf], 0x0a
    lea rsi, [rel char_buf]
    mov rdi, 1
    mov rdx, 1
    call sys_write
    pop rdi
    pop rsi
    ret

; ── string_length ───────────────────────────────────────────────────────────────
; Returns length of null-terminated string.
; Arguments:
;   rdi = string pointer
; Returns:
;   rax = length (not counting terminator)
string_length:
    push rcx
    push rdi
    mov rdi, rdi
    xor rax, rax
    mov rcx, -1
    repne scasb
    not rcx
    dec rcx
    mov rax, rcx
    pop rdi
    pop rcx
    ret

; ── string_equal ────────────────────────────────────────────────────────────────
; Compare two null-terminated strings.
; Arguments:
;   rdi = string 1
;   rsi = string 2
; Returns:
;   rax = 1 if equal, 0 if not
string_equal:
    push rbx
    push rcx
    push rdi
    push rsi

.compare_loop:
    mov al, [rdi]
    mov bl, [rsi]
    cmp al, bl
    jne .not_equal
    test al, al
    jz .equal
    inc rdi
    inc rsi
    jmp .compare_loop

.equal:
    mov rax, 1
    jmp .done_equal

.not_equal:
    xor rax, rax

.done_equal:
    pop rsi
    pop rdi
    pop rcx
    pop rbx
    ret

; ── string_copy ─────────────────────────────────────────────────────────────────
; Copy null-terminated string with limit.
; Arguments:
;   rsi = source
;   rdi = destination
;   rcx = max bytes
string_copy:
    push rsi
    push rdi
    push rcx

.copy_loop:
    test rcx, rcx
    jz .copy_done
    mov al, [rsi]
    mov [rdi], al
    test al, al
    jz .copy_done
    inc rsi
    inc rdi
    dec rcx
    jmp .copy_loop

.copy_done:
    pop rcx
    pop rdi
    pop rsi
    ret

; ── print_decimal ───────────────────────────────────────────────────────────────
; Print a non-negative integer as decimal.
; Arguments:
;   rdi = value to print
print_decimal:
    push rbx
    push rcx
    push rdx
    push rdi
    push rsi

    mov rax, rdi
    mov rbx, 10
    lea rcx, [rel dec_buf + 15]
    mov byte [rcx], 0
    dec rcx

.convert_loop:
    xor rdx, rdx
    div rbx
    add dl, '0'
    mov [rcx], dl
    dec rcx
    test rax, rax
    jnz .convert_loop

    inc rcx                                ; point to first digit

    mov rsi, rcx
    mov rdi, 1
    call write_string

    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rbx
    ret

; ── write_escaped ───────────────────────────────────────────────────────────────
; Write a string with escape sequences decoded.
; Arguments:
;   rdi = string to write with escapes
write_escaped:
    push rbx
    push r12
    push r13

    mov r12, rdi                           ; r12 = current position

.esc_loop:
    movzx rax, byte [r12]
    test al, al
    jz .esc_done

    cmp al, '\'
    je .handle_escape

    ; Regular character
    mov [rel char_buf], al
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    inc r12
    jmp .esc_loop

.handle_escape:
    inc r12                                ; skip backslash
    movzx rax, byte [r12]
    test al, al
    jz .esc_done                           ; trailing backslash

    cmp al, 'n'
    je .esc_newline
    cmp al, 't'
    je .esc_tab
    cmp al, 'r'
    je .esc_cr
    cmp al, 0x5c
    je .esc_backslash
    cmp al, '0'
    je .esc_octal
    cmp al, 'a'
    je .esc_bell
    cmp al, 'b'
    je .esc_bs
    cmp al, 'e'
    je .esc_esc
    cmp al, 'f'
    je .esc_ff
    cmp al, 'v'
    je .esc_vt

    ; Unknown escape: emit literal backslash + char
    mov byte [rel char_buf], '\'
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov [rel char_buf], al
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    inc r12
    jmp .esc_loop

.esc_newline:
    mov byte [rel char_buf], 0x0d
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    mov byte [rel char_buf], 0x0a
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    inc r12
    jmp .esc_loop

.esc_tab:
    mov byte [rel char_buf], 0x09
    jmp .esc_write_byte

.esc_cr:
    mov byte [rel char_buf], 0x0d
    jmp .esc_write_byte

.esc_backslash:
    mov byte [rel char_buf], '\'
    jmp .esc_write_byte

.esc_bell:
    mov byte [rel char_buf], 0x07
    jmp .esc_write_byte

.esc_bs:
    mov byte [rel char_buf], 0x08
    jmp .esc_write_byte

.esc_esc:
    mov byte [rel char_buf], 0x1b
    jmp .esc_write_byte

.esc_ff:
    mov byte [rel char_buf], 0x0c
    jmp .esc_write_byte

.esc_vt:
    mov byte [rel char_buf], 0x0b
    jmp .esc_write_byte

.esc_octal:
    ; Parse \0NNN (up to 3 octal digits)
    push r12
    xor rbx, rbx
    mov rcx, 3

.oct_loop:
    inc r12
    movzx rax, byte [r12]
    cmp al, '0'
    jb .oct_done
    cmp al, '7'
    ja .oct_done
    shl rbx, 3
    sub al, '0'
    add rbx, rax
    dec rcx
    jnz .oct_loop

.oct_done:
    mov [rel char_buf], bl
    ; r12 is already past the octal digits
    jmp .esc_write_byte_keep_pos

.esc_write_byte:
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    inc r12
    jmp .esc_loop

.esc_write_byte_keep_pos:
    lea rdi, [rel char_buf]
    mov rsi, 1
    mov rdi, 1
    call sys_write
    jmp .esc_loop

.esc_done:
    pop r13
    pop r12
    pop rbx
    ret

; ── parse_decimal ───────────────────────────────────────────────────────────────
; Parse a decimal integer from a string.
; Arguments:
;   rsi = string pointer
; Returns:
;   rax = parsed value (0 if invalid)
parse_decimal:
    push rbx
    push rcx
    push rsi

    xor rax, rax
    xor rcx, rcx
    mov rbx, 10

.loop:
    movzx rcx, byte [rsi]
    test rcx, rcx
    jz .done
    cmp rcx, '0'
    jb .done
    cmp rcx, '9'
    ja .done

    mul rbx
    sub rcx, '0'
    add rax, rcx
    inc rsi
    jmp .loop

.done:
    pop rsi
    pop rcx
    pop rbx
    ret

; ── prefix_match ────────────────────────────────────────────────────────────────
; Check if string starts with a given prefix.
; Arguments:
;   rdi = string
;   rsi = prefix
; Returns:
;   rax = 1 if match, 0 if not
prefix_match:
    push rbx
    push rdi
    push rsi

.loop:
    mov al, [rsi]
    test al, al
    jz .match                              ; prefix exhausted → match
    mov bl, [rdi]
    cmp al, bl
    jne .no_match
    inc rdi
    inc rsi
    jmp .loop

.match:
    mov rax, 1
    jmp .done_prefix

.no_match:
    xor rax, rax

.done_prefix:
    pop rsi
    pop rdi
    pop rbx
    ret

; ── Syscall wrappers ────────────────────────────────────────────────────────────

; sys_exit(code) — rax=0
sys_exit:
    xor rdi, rdi
    mov rax, 0
    syscall

; sys_write(fd, buf, len) — rax=1
sys_write:
    mov rax, 1
    syscall
    ret

; sys_read(fd, buf, len) — rax=2
sys_read:
    mov rax, 2
    syscall
    ret

; sys_fork() — rax=8
; Returns: child PID in parent, 0 in child
sys_fork:
    mov rax, 8
    syscall
    ret

; sys_exec(path, argv, envp) — rax=9
; rdi=path, rsi=argv, rdx=envp
; On success does not return. On error returns negative errno.
sys_exec:
    mov rax, 9
    syscall
    ret

; sys_waitpid(pid, wstatus, flags) — rax=10
; rdi=pid, rsi=&status, rdx=flags
sys_waitpid:
    mov rax, 10
    syscall
    ret

; ── External command helpers ──────────────────────────────────────────────────

; ── try_find_and_exec ─────────────────────────────────────────────────────────
; Try to find and execute an external command via fork/exec.
; Arguments:
;   rdi = command name (argv[0])
;   rsi = argv array
;   rdx = envp array
; Returns:
;   rax = 0 if command was found and executed, 1 if not found
; Preserves: r12, r13, r14, r15, rbx (callee-saved)
try_find_and_exec:
    push rbx
    push r12
    push r13
    push r14
    push r15

    mov rbx, rdi                           ; rbx = cmd_name
    mov r12, rsi                           ; r12 = argv
    mov r13, rdx                           ; r13 = envp

    ; Save state to memory (syscalls clobber all registers)
    mov [rel saved_cmd_name], rbx
    mov [rel saved_argv], r12
    mov [rel saved_envp], r13

    ; Check if cmd_name contains '/'
    mov rdi, rbx
    call path_contains_slash
    cmp rax, 0
    jne .try_direct

    ; ── Try /bin/<cmd> ──
.try_bin:
    mov rbx, [rel saved_cmd_name]
    mov r12, [rel saved_argv]
    mov r13, [rel saved_envp]

    lea rdi, [rel str_path_bin]
    mov rsi, rbx
    lea rdx, [rel path_buffer]
    call join_path

    ; Check if file exists via open()
    mov rbx, [rel saved_cmd_name]
    mov r12, [rel saved_argv]
    mov r13, [rel saved_envp]

    lea rdi, [rel path_buffer]
    xor rsi, rsi                           ; O_RDONLY
    mov rax, 12                            ; open
    syscall

    cmp rax, 0
    jl .try_sbin                           ; not found

    ; File exists — close fd and continue
    mov rdi, rax
    mov rax, 13                            ; close
    syscall

    mov rbx, [rel saved_cmd_name]
    mov r12, [rel saved_argv]
    mov r13, [rel saved_envp]
    lea r14, [rel path_buffer]
    jmp .do_fork_exec

    ; ── Try /sbin/<cmd> ──
.try_sbin:
    mov rbx, [rel saved_cmd_name]
    mov r12, [rel saved_argv]
    mov r13, [rel saved_envp]

    lea rdi, [rel str_path_sbin]
    mov rsi, rbx
    lea rdx, [rel path_buffer]
    call join_path

    mov rbx, [rel saved_cmd_name]
    mov r12, [rel saved_argv]
    mov r13, [rel saved_envp]

    lea rdi, [rel path_buffer]
    xor rsi, rsi
    mov rax, 12
    syscall

    cmp rax, 0
    jl .not_found

    mov rdi, rax
    mov rax, 13
    syscall

    mov rbx, [rel saved_cmd_name]
    mov r12, [rel saved_argv]
    mov r13, [rel saved_envp]
    lea r14, [rel path_buffer]
    jmp .do_fork_exec

    ; ── Use cmd_name directly (contains '/') ──
.try_direct:
    mov r14, rbx                           ; path = cmd_name as-is

    ; ── Fork and exec ──
.do_fork_exec:
    ; r14 = path to executable
    ; r12 = argv
    ; r13 = envp

    ; Push values onto stack so child can pop them after fork
    ; (fork syscall clobbers all registers except rax)
    push r13                               ; envp
    push r12                               ; argv
    push r14                               ; path

    mov rax, 8                             ; fork
    syscall

    cmp rax, 0
    je .child

    ; ── Parent process ──
    ; rax = child PID
    mov rdi, rax
    sub rsp, 8                             ; space for exit status
    mov rsi, rsp
    xor rdx, rdx                           ; flags = 0
    mov rax, 10                            ; waitpid
    syscall
    add rsp, 8                             ; clean up status

    ; Discard the 3 pushed values
    add rsp, 24

    xor rax, rax                           ; return 0 = success
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

    ; ── Child process ──
.child:
    ; Restore path, argv, envp from stack
    pop rdi                                ; path
    pop rsi                                ; argv
    pop rdx                                ; envp
    mov rax, 9                             ; exec
    syscall

    ; If exec returns, it failed — exit with code 1
    mov rdi, 1
    xor rax, rax
    syscall                                 ; exit(1) — never returns

.not_found:
    mov rax, 1                             ; return 1 = not found
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; ── path_contains_slash ───────────────────────────────────────────────────────
; Check if a null-terminated string contains '/'.
; Arguments:
;   rdi = string
; Returns:
;   rax = 1 if found, 0 if not
; Preserves all registers (save/restore rdi)
path_contains_slash:
    push rdi
.loop:
    mov al, [rdi]
    test al, al
    jz .not_found_s
    cmp al, '/'
    je .found_s
    inc rdi
    jmp .loop
.not_found_s:
    xor rax, rax
    pop rdi
    ret
.found_s:
    mov rax, 1
    pop rdi
    ret

; ── join_path ─────────────────────────────────────────────────────────────────
; Concatenate prefix + name into destination buffer, null-terminated.
; Arguments:
;   rdi = prefix (e.g. "/bin/")
;   rsi = name   (e.g. "echo")
;   rdx = dest buffer
; Preserves: rbx, r12-r15
join_path:
    push rbx
    push r12
    push r13

    mov rbx, rdx                           ; rbx = dest
    mov r12, rdi                           ; r12 = prefix
    mov r13, rsi                           ; r13 = name

.copy_prefix:
    mov al, [r12]
    test al, al
    jz .copy_name
    mov [rbx], al
    inc rbx
    inc r12
    jmp .copy_prefix

.copy_name:
    mov al, [r13]
    test al, al
    jz .done_join
    mov [rbx], al
    inc rbx
    inc r13
    jmp .copy_name

.done_join:
    mov byte [rbx], 0                      ; null-terminate

    pop r13
    pop r12
    pop rbx
    ret

; ── Data section ────────────────────────────────────────────────────────────────

; Command table: pairs of (name_ptr, handler_ptr), null-terminated
command_table:
    dq cmd_cat,     builtin_cat
    dq cmd_clear,   builtin_clear
    dq cmd_echo,    builtin_echo
    dq cmd_false,   builtin_false
    dq cmd_printenv, builtin_printenv
    dq cmd_true,    builtin_true
    dq 0, 0                                 ; table terminator

; ── Strings ────────────────────────────────────────────────────────────────────
str_prompt:      db PROMPT_TEXT, 0
str_welcome:     db "vish -- VIBIX SHell", 0x0d, 0x0a, 0x0d, 0x0a, 0
str_farewell:    db 0x0d, 0x0a, "goodbye", 0x0d, 0x0a, 0
str_not_found:   db ": command not found", 0
str_exit_usage:  db "exit: usage: exit [code]", 0
str_no_history:  db "no history", 0
str_cat_notimpl: db "cat: file I/O not yet implemented", 0
str_path_bin:    db "/bin/", 0
str_path_sbin:   db "/sbin/", 0

cmd_exit:        db "exit", 0
cmd_help:        db "help", 0
cmd_history_str: db "history", 0
cmd_clear:       db "clear", 0
cmd_echo:        db "echo", 0
cmd_cat:         db "cat", 0
cmd_printenv:    db "printenv", 0
cmd_true:        db "true", 0
cmd_false:       db "false", 0

help_text:
    db "vish -- VIBIX SHell", 0x0d, 0x0a
    db 0x0d, 0x0a
    db "Built-in commands:", 0x0d, 0x0a
    db "  cat       concatenate and display files", 0x0d, 0x0a
    db "  clear     clear the terminal screen", 0x0d, 0x0a
    db "  echo      write arguments to stdout", 0x0d, 0x0a
    db "  exit      exit the shell", 0x0d, 0x0a
    db "  false     return false (exit code 1)", 0x0d, 0x0a
    db "  help      display this help message", 0x0d, 0x0a
    db "  history   display command history", 0x0d, 0x0a
    db "  printenv  print environment variables", 0x0d, 0x0a
    db "  true      return true (exit code 0)", 0x0d, 0x0a
    db 0x0d, 0x0a
    db "See documentation for more information.", 0x0d, 0x0a
    db 0

; ── Uninitialized data (BSS) ───────────────────────────────────────────────────
align 16
line_buffer:     resb LINE_CAPACITY
argv_table:      resq ARGV_CAPACITY
history_buffer:  resb HISTORY_SLOTS * HISTORY_STRIDE
read_buffer:     resb READ_CHUNK

char_buf:        resb 8                    ; scratch for single char I/O
dec_buf:         resb 16                   ; scratch for decimal conversion

exit_flag:       resb 1                    ; 0 = running, 1 = exit requested
exit_code_storage: resq 1
saved_envp:      resq 1                    ; stored envp pointer
history_count:   resq 1                    ; number of entries
history_index:   resq 1                    ; next write position
history_browse:  resq 1                    ; browse position (0 = not browsing)

; ── External command execution state ──────────────────────────────────────────
; (saved_envp reused for try_find_and_exec — stored at shell_init)
path_buffer:     resb 256                  ; buffer for building paths
saved_cmd_name:  resq 1                    ; temp for cmd_name across syscalls
saved_argv:      resq 1                    ; temp for argv across syscalls
