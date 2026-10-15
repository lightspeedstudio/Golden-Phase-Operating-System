// Fixed 32-bit instruction decoder (matches kairos VM opcodes).

module phase_decoder (
    input  wire [31:0] instr,
    output wire [4:0]  opcode,
    output wire [1:0]  ra,
    output wire [1:0]  rb,
    output wire [1:0]  rc,
    output wire [1:0]  imm_phase,
    output wire [15:0] addr,
    output wire [21:0] target
);
    assign opcode    = instr[31:27];
    assign ra        = instr[26:25];
    assign rb        = instr[24:23];
    assign rc        = instr[22:21];
    assign imm_phase = instr[20:19];
    assign addr      = instr[15:0];
    assign target    = instr[21:0];
endmodule
