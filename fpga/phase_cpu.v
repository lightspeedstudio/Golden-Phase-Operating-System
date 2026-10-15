// Phase CPU — LOAD / COMPOSE / REFLECT / HALT (matches Rust encode.rs opcodes).
// Register writes are same-cycle (combinational wen into RF), then PC advances.

module phase_cpu #(
    parameter IMEM_DEPTH = 64
) (
    input  wire        clk,
    input  wire        rst,
    input  wire        run,
    output reg         halted,
    output wire [1:0]  reg_a,
    output wire [1:0]  reg_b,
    output wire [1:0]  reg_c,
    output wire [1:0]  reg_d,
    output wire [31:0] trace_pc,
    output wire [4:0]  trace_opcode,
    output wire [1:0]  trace_topo,
    output wire [15:0] trace_count
);
    reg [31:0] pc;
    reg [31:0] imem [0:IMEM_DEPTH-1];

    wire [31:0] instr = imem[pc[5:0]];
    wire [4:0]  opcode;
    wire [1:0]  ra, rb, rc, imm_phase;
    wire [15:0] addr;
    wire [21:0] target;

    phase_decoder u_dec (
        .instr(instr),
        .opcode(opcode),
        .ra(ra), .rb(rb), .rc(rc),
        .imm_phase(imm_phase),
        .addr(addr),
        .target(target)
    );

    wire [1:0] rf0, rf1, rf2, rf3;

    function automatic [1:0] rf_sel(input [1:0] sel);
        case (sel)
            2'd0: rf_sel = rf0;
            2'd1: rf_sel = rf1;
            2'd2: rf_sel = rf2;
            2'd3: rf_sel = rf3;
            default: rf_sel = 2'd0;
        endcase
    endfunction

    wire [1:0] op_a = rf_sel(ra);
    wire [1:0] op_b = rf_sel(rb);

    wire [1:0] alu_out, reflect_out;
    wire t_sign, t_energy;
    phase_alu u_alu (
        .a(op_a), .b(op_b),
        .compose_out(alu_out), .reflect_out(reflect_out),
        .t_sign_out(t_sign), .t_energy_out(t_energy)
    );

    wire        reg_wen;
    wire [1:0]  reg_wreg;
    wire [1:0]  reg_wdata;

    assign reg_wen = run && !halted && !rst && (
        (opcode == 5'd0) || (opcode == 5'd1) || (opcode == 5'd2) ||
        (opcode == 5'd4) || (opcode == 5'd5)
    );
    assign reg_wreg = (opcode == 5'd0) ? ra :
                      (opcode == 5'd1) ? rc :
                      (opcode == 5'd2) ? rb :
                      (opcode == 5'd4) ? rb :
                      (opcode == 5'd5) ? rb : 2'd0;
    assign reg_wdata = (opcode == 5'd0) ? imm_phase :
                       (opcode == 5'd1) ? alu_out :
                       (opcode == 5'd2) ? reflect_out :
                       (opcode == 5'd4) ? {1'b0, t_sign} :
                       (opcode == 5'd5) ? {1'b0, t_energy} : 2'd0;

    phase_registers u_regs (
        .clk(clk), .rst(rst),
        .wen(reg_wen), .wreg(reg_wreg), .wdata(reg_wdata),
        .ra(rf0), .rb(rf1), .rc(rf2), .rd(rf3)
    );

    assign reg_a = rf0;
    assign reg_b = rf1;
    assign reg_c = rf2;
    assign reg_d = rf3;

    wire [1:0] topo_L, topo_R, topo_status;
    topology_unit u_topo (
        .a(rf0), .b(rf1), .c(rf2),
        .check_L(topo_L), .check_R(topo_R), .status(topo_status)
    );

    trace_bus u_trace (
        .clk(clk), .rst(rst), .push(run && !halted),
        .pc({16'd0, pc[15:0]}), .opcode(opcode),
        .phase_a(rf0), .phase_b(rf1), .topo_status(topo_status),
        .trace_pc(trace_pc), .trace_opcode(trace_opcode),
        .trace_topo(trace_topo), .trace_count(trace_count)
    );

    // Complex VM: LOAD A3, LOAD B1, COMPOSE->C(=2), THRESHOLD A->D(=1),
    // ENERGY C->B(=0), REFLECT C->A(=1), HALT  => A=1 B=0 C=2 D=1
    integer i;
    initial begin
        for (i = 0; i < IMEM_DEPTH; i = i + 1) imem[i] = {5'd7, 27'd0};
        imem[0] = {5'd0, 2'd0, 2'd0, 2'd0, 2'd3, 19'd0}; // LOAD A Phi3
        imem[1] = {5'd0, 2'd1, 2'd0, 2'd0, 2'd1, 19'd0}; // LOAD B Phi1
        imem[2] = {5'd1, 2'd0, 2'd1, 2'd2, 2'd0, 19'd0}; // COMPOSE A,B->C
        imem[3] = {5'd4, 2'd0, 2'd3, 23'd0};             // THRESHOLD A->D
        imem[4] = {5'd5, 2'd2, 2'd1, 23'd0};             // ENERGY C->B
        imem[5] = {5'd2, 2'd2, 2'd0, 2'd0, 2'd0, 19'd0}; // REFLECT C->A
        imem[6] = {5'd7, 27'd0};                         // HALT
    end

    always @(posedge clk) begin
        if (rst) begin
            pc     <= 32'd0;
            halted <= 1'b0;
        end else if (run && !halted) begin
            if (opcode == 5'd7)
                halted <= 1'b1;
            else
                pc <= pc + 1;
        end
    end
endmodule
