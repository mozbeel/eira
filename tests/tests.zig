const std = @import("std");
const io = std.testing.io;
const alloc = std.testing.allocator;
const RunResult = std.process.RunResult;

fn run_lua(file: []const u8) !RunResult {
    const result = try std.process.run(alloc, io, .{
        .argv = &.{
            "zig-out/bin/lua",
            file,
        },
    });

    std.debug.print("Output (\"{s}\"):\n{s}\n", .{ file, result.stdout });

    return result;
}

fn free_lua(result: RunResult) void {
    alloc.free(result.stderr);
    alloc.free(result.stdout);
}

test "Hello World" {
    const result = try run_lua("tests/hello_world.lua");

    defer free_lua(result);
}

test "Coroutines" {
    const result = try run_lua("tests/coroutines.lua");

    defer free_lua(result);
}

test "Loops" {
    const result = try run_lua("tests/loops.lua");

    defer free_lua(result);
}

test "All Syntax" {
    const result = try run_lua("tests/all_syntax.lua");

    defer free_lua(result);
}

test "Eira Functions" {
    const result = try run_lua("tests/eira/functions.lua");

    defer free_lua(result);
}
