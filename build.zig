const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const lib_mod = b.addModule("lib", .{
        .target = target,
        .optimize = optimize,
        .link_libc = true,
    });

    lib_mod.addCSourceFiles(.{
        .files = &[_][]const u8{
            "src/c/lapi.c",
            "src/c/lcode.c",
            "src/c/lctype.c",
            "src/c/ldebug.c",
            "src/c/ldo.c",
            "src/c/ldump.c",
            "src/c/lfunc.c",
            "src/c/lgc.c",
            "src/c/llex.c",
            "src/c/lmem.c",
            "src/c/lobject.c",
            "src/c/lopcodes.c",
            "src/c/lparser.c",
            "src/c/lstate.c",
            "src/c/lstring.c",
            "src/c/ltable.c",
            "src/c/ltm.c",
            "src/c/lundump.c",
            "src/c/lvm.c",
            "src/c/lzio.c",
            "src/c/lauxlib.c",
            "src/c/lbaselib.c",
            "src/c/lcorolib.c",
            "src/c/ldblib.c",
            "src/c/liolib.c",
            "src/c/lmathlib.c",
            "src/c/loadlib.c",
            "src/c/loslib.c",
            "src/c/lstrlib.c",
            "src/c/ltablib.c",
            "src/c/lutf8lib.c",
            "src/c/linit.c",
        },
    });

    lib_mod.addIncludePath(b.path("src"));

    const lib = b.addLibrary(.{
        .name = "lib",
        .root_module = lib_mod,
        .linkage = .static,
    });

    const lua_mod = b.addModule("lua", .{
        .target = target,
        .optimize = optimize,
        .link_libc = true,
    });

    lua_mod.addIncludePath(b.path("src/c"));
    lua_mod.addCSourceFile(.{
        .file = b.path("src/c/lua.c"),
    });

    lua_mod.linkLibrary(lib);

    const lua = b.addExecutable(.{
        .name = "lua",
        .root_module = lua_mod,
    });

    b.installArtifact(lua);

    const luac_mod = b.addModule("luac", .{
        .target = target,
        .optimize = optimize,
        .link_libc = true,
    });

    luac_mod.addIncludePath(b.path("src/c"));
    luac_mod.addCSourceFile(.{
        .file = b.path("src/c/luac.c"),
    });

    lua_mod.linkLibrary(lib);

    const luac = b.addExecutable(.{
        .name = "luac",
        .root_module = lua_mod,
    });

    b.installArtifact(luac);

    const lua_run_exe = b.addRunArtifact(lua);
    const lua_run_step = b.step("run_lua", "Run the application");
    lua_run_step.dependOn(&lua_run_exe.step);

    const luac_run_exe = b.addRunArtifact(luac);
    const luac_run_step = b.step("run_luac", "Run the application");
    luac_run_step.dependOn(&luac_run_exe.step);

    const test_step = b.step("test", "Run unit tests");

    const unit_tests = b.addTest(.{
        .root_module = b.createModule(.{
            .root_source_file = b.path("tests/tests.zig"),
            .target = target,
        }),
    });

    const run_unit_tests = b.addRunArtifact(unit_tests);
    test_step.dependOn(&run_unit_tests.step);
}
