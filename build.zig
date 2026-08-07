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
            "src/lapi.c",
            "src/lcode.c",
            "src/lctype.c",
            "src/ldebug.c",
            "src/ldo.c",
            "src/ldump.c",
            "src/lfunc.c",
            "src/lgc.c",
            "src/llex.c",
            "src/lmem.c",
            "src/lobject.c",
            "src/lopcodes.c",
            "src/lparser.c",
            "src/lstate.c",
            "src/lstring.c",
            "src/ltable.c",
            "src/ltm.c",
            "src/lundump.c",
            "src/lvm.c",
            "src/lzio.c",
            "src/lauxlib.c",
            "src/lbaselib.c",
            "src/lcorolib.c",
            "src/ldblib.c",
            "src/liolib.c",
            "src/lmathlib.c",
            "src/loadlib.c",
            "src/loslib.c",
            "src/lstrlib.c",
            "src/ltablib.c",
            "src/lutf8lib.c",
            "src/linit.c",
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

    lua_mod.addIncludePath(b.path("src"));
    lua_mod.addCSourceFile(.{
        .file = b.path("src/lua.c"),
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

    luac_mod.addIncludePath(b.path("src"));
    luac_mod.addCSourceFile(.{
        .file = b.path("src/luac.c"),
    });

    lua_mod.linkLibrary(lib);

    const luac = b.addExecutable(.{
        .name = "luac",
        .root_module = lua_mod,
    });

    b.installArtifact(luac);
}
