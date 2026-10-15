// Phase ALU — compose, reflect, thresholds (matches gphase_core 4x4 table).

module phase_alu (
    input  wire [1:0] a,
    input  wire [1:0] b,
    output wire [1:0] compose_out,
    output wire [1:0] reflect_out,
    output wire       t_sign_out,
    output wire       t_energy_out
);
    function automatic [1:0] compose_func(input [1:0] aa, input [1:0] bb);
        case ({aa, bb})
            4'b0000: compose_func = 2'd0; 4'b0001: compose_func = 2'd1; 4'b0010: compose_func = 2'd2; 4'b0011: compose_func = 2'd3;
            4'b0100: compose_func = 2'd1; 4'b0101: compose_func = 2'd3; 4'b0110: compose_func = 2'd3; 4'b0111: compose_func = 2'd2;
            4'b1000: compose_func = 2'd2; 4'b1001: compose_func = 2'd3; 4'b1010: compose_func = 2'd3; 4'b1011: compose_func = 2'd1;
            4'b1100: compose_func = 2'd3; 4'b1101: compose_func = 2'd2; 4'b1110: compose_func = 2'd1; 4'b1111: compose_func = 2'd0;
            default: compose_func = 2'd0;
        endcase
    endfunction

    function automatic [1:0] reflect_func(input [1:0] aa);
        case (aa)
            2'd0: reflect_func = 2'd3;
            2'd1: reflect_func = 2'd2;
            2'd2: reflect_func = 2'd1;
            2'd3: reflect_func = 2'd0;
            default: reflect_func = 2'd0;
        endcase
    endfunction

    assign compose_out  = compose_func(a, b);
    assign reflect_out  = reflect_func(a);
    assign t_sign_out   = (a == 2'd2) || (a == 2'd3);
    assign t_energy_out = (a == 2'd0) || (a == 2'd3);
endmodule
