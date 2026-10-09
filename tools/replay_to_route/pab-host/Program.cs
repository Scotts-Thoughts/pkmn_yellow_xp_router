// pab-host: Poke-A-Byte's mapper engine, one read at a time.
//
//   pab-host --mappers <mapper folder> --mapper <path relative to it, e.g. STANDARD/gen5/pokemon_black.xml>
//
// The memory comes from stdin instead of an emulator, so a replay can be fed through the real
// mapper (XML + JavaScript) as fast as it is decoded. Every message on stdout is a u32 (LE)
// length followed by that many bytes of UTF-8 JSON; logs go to stderr.
//
// On start-up the host writes {"blocks": [[start, length], ...], "gameName", "platform"}: the
// memory blocks the mapper reads (Poke-A-Byte's transfer blocks). Then, from stdin:
//
//   'S' u32 run_count { u32 block, u32 offset, u32 length, bytes } * run_count
//       Patch the held memory, run one Poke-A-Byte read (two for the first one, like
//       StartProcessing) and answer with the properties it reported changed:
//       [{"path", "address", "value", "bytes", "isFrozen", "fieldsChanged"}, ...]
//       (with --compact: only value and address changes, and no "bytes")
//   'M' Answer with the mapper as GET /mapper serves it ({"meta", "properties", "glossary"}).
//   'R' Start over with a fresh Poke-A-Byte instance (the mapper loaded again, its script state
//       gone, the held memory zeroed) and answer {"reset": true}.
//   'X' Export the script state: {"variables", "state", "values"} (the mapper's __variables and
//       __state, and the values of the properties the script sets: those without an address).
//   'I' u32 length, JSON: import the variables and values 'X' exported into this instance (not
//       __state, see ScriptState.Import); answers {"imported": n}.
//       A fresh instance seeded this way continues like the one that exported it would have
//       (caches the script built up, state machines), as far as that state is plain data.
//   'Q' Quit.
//
// Anything that goes wrong is answered with {"error": "..."}.

using System.Buffers.Binary;
using System.Text;
using System.Text.Json;
using Microsoft.Extensions.Logging;
using PokeAByte.Domain.Interfaces;
using PokeAByte.Domain.Logic;
using PokeAByte.Domain.Models;

namespace PabHost;

/// The memory the host holds; Poke-A-Byte reads it like an emulator's.
sealed class StdinDriver : IPokeAByteDriver
{
    public byte[][] Blocks = [];
    public uint[] Starts = [];
    public bool Configured;

    public string ProperName => "replay_to_route";
    public int DelayMsBetweenReads => 0;
    public bool SupportsFreeze => false;

    public Task EstablishConnection() => Task.CompletedTask;
    public Task Disconnect() => Task.CompletedTask;

    public ValueTask ReadBytes(BlockData[] transferBlocks)
    {
        if (!Configured)
        {
            Starts = transferBlocks.Select(b => (uint)b.Start).ToArray();
            Blocks = transferBlocks.Select(b => new byte[b.Data.Length]).ToArray();
            Configured = true;
            throw new ProbeException();
        }
        for (int i = 0; i < transferBlocks.Length; i++)
        {
            Blocks[i].AsSpan().CopyTo(transferBlocks[i].Data.Span);
        }
        return ValueTask.CompletedTask;
    }

    // Mappers write to the game (containerprocessor, freezes); a replay cannot be changed.
    public ValueTask WriteBytes(uint startingMemoryAddress, byte[] values, string? path = null) => ValueTask.CompletedTask;
}

sealed class ProbeException : Exception { }

/// The plain-data part of a mapper script's state, moved between instances.
static class ScriptState
{
    public static byte[] Export(PokeAByteInstance instance, JsonSerializerOptions options)
    {
        using var ms = new MemoryStream();
        using (var w = new Utf8JsonWriter(ms))
        {
            w.WriteStartObject();
            WriteDict(w, "variables", instance.Variables, options);
            WriteDict(w, "state", instance.State, options);
            w.WriteStartObject("values");
            foreach (var p in instance.Mapper.Properties.Values)
            {
                if (p.Address != null || p.Value == null) continue;
                TryWrite(w, p.Path, p.Value, options);
            }
            w.WriteEndObject();
            w.WriteEndObject();
        }
        return ms.ToArray();
    }

    static void WriteDict(Utf8JsonWriter w, string name, Dictionary<string, object?> dict, JsonSerializerOptions options)
    {
        w.WriteStartObject(name);
        foreach (var (k, v) in dict) TryWrite(w, k, v, options);
        w.WriteEndObject();
    }

    /// Values that do not serialise (references to engine objects) are left out.
    static void TryWrite(Utf8JsonWriter w, string key, object? value, JsonSerializerOptions options)
    {
        byte[] json;
        try { json = JsonSerializer.SerializeToUtf8Bytes(value, options); }
        catch (Exception) { return; }
        w.WritePropertyName(key);
        w.WriteRawValue(json, skipInputValidation: true);
    }

    /// JSON as the script engine hands plain data to .NET: objects as ExpandoObjects, arrays as
    /// object arrays, numbers as doubles.
    static object? FromJson(JsonElement e) => e.ValueKind switch
    {
        JsonValueKind.Object => ToExpando(e),
        JsonValueKind.Array => e.EnumerateArray().Select(FromJson).ToArray(),
        JsonValueKind.String => e.GetString(),
        JsonValueKind.Number => e.GetDouble(),
        JsonValueKind.True => true,
        JsonValueKind.False => false,
        _ => null,
    };

    static object ToExpando(JsonElement e)
    {
        IDictionary<string, object?> d = new System.Dynamic.ExpandoObject();
        foreach (var p in e.EnumerateObject()) d[p.Name] = FromJson(p.Value);
        return d;
    }

    /// A property value in the property's own type.
    static object? ForProperty(IPokeAByteProperty p, JsonElement e)
    {
        try
        {
            return (p.Type, e.ValueKind) switch
            {
                (_, JsonValueKind.Null) => null,
                (PropertyType.Int, JsonValueKind.Number) => e.TryGetInt32(out var i) ? i : (object)e.GetDouble(),
                (PropertyType.Uint, JsonValueKind.Number) => e.TryGetUInt32(out var u) ? u : (object)e.GetDouble(),
                (PropertyType.BinaryCodedDecimal, JsonValueKind.Number) => e.GetInt32(),
                (PropertyType.Bool or PropertyType.Bit, JsonValueKind.True or JsonValueKind.False) => e.GetBoolean(),
                (PropertyType.BitArray, JsonValueKind.Array) => e.EnumerateArray().Select(x => x.ValueKind == JsonValueKind.True).ToArray(),
                (PropertyType.ByteArray, JsonValueKind.Array) => e.EnumerateArray().Select(x => (byte)x.GetInt32()).ToArray(),
                (_, JsonValueKind.String) => e.GetString(),
                _ => FromJson(e),
            };
        }
        catch (Exception)
        {
            return FromJson(e);
        }
    }

    public static int Import(PokeAByteInstance instance, byte[] json)
    {
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;
        int n = 0;
        if (root.TryGetProperty("variables", out var vars))
        {
            foreach (var p in vars.EnumerateObject()) { instance.Variables[p.Name] = FromJson(p.Value); n++; }
        }
        // __state is not imported: scripts use it for one-time setup of things that live in the
        // instance (the gen 5 mappers' memory containers behind `__state.initialized`), which a
        // fresh instance must do itself.
        if (root.TryGetProperty("values", out var values))
        {
            foreach (var p in values.EnumerateObject())
            {
                if (instance.Mapper.Properties.TryGetValue(p.Name, out var prop))
                {
                    prop.Value = ForProperty(prop, p.Value);
                    n++;
                }
            }
        }
        return n;
    }
}

/// Collects what Poke-A-Byte would push to its SignalR clients.
sealed class Notifier : IClientNotifier
{
    public readonly JsonSerializerOptions Options;
    public byte[]? Changes;
    public readonly List<string> Errors = [];
    /// --compact: only changes of a value or an address, without the bytes (nothing a
    /// recorder reads; the rest is most of the volume).
    public bool Compact;

    public Notifier(JsonSerializerOptions options) { Options = options; }

    public Task SendInstanceReset() => Task.CompletedTask;
    public Task SendMapperLoaded(IPokeAByteMapper mapper) => Task.CompletedTask;

    public Task SendError(IProblemDetails problemDetails)
    {
        Errors.Add($"{problemDetails.Title}: {problemDetails.Detail}");
        return Task.CompletedTask;
    }

    public Task SendPropertiesChanged(IList<IPokeAByteProperty> properties)
    {
        // Serialised now: Poke-A-Byte clears FieldsChanged once this returns.
        using var ms = new MemoryStream();
        using (var w = new Utf8JsonWriter(ms))
        {
            w.WriteStartArray();
            foreach (var p in properties)
            {
                if (Compact && (p.FieldsChanged & (FieldChanges.Value | FieldChanges.Address)) == 0) continue;
                w.WriteStartObject();
                w.WriteString("path", p.Path);
                if (p.Address is uint a) w.WriteNumber("address", a); else w.WriteNull("address");
                w.WritePropertyName("value");
                JsonSerializer.Serialize(w, p.Value, Options);
                if (!Compact)
                {
                    w.WritePropertyName("bytes");
                    JsonSerializer.Serialize(w, p.Bytes, Options);
                }
                w.WriteBoolean("isFrozen", p.IsFrozen);
                w.WritePropertyName("fieldsChanged");
                JsonSerializer.Serialize(w, p.FieldsChanged, Options);
                w.WriteEndObject();
            }
            w.WriteEndArray();
        }
        Changes = ms.ToArray();
        return Task.CompletedTask;
    }
}

sealed class StderrLogger<T> : ILogger<T>
{
    private readonly LogLevel _min;
    public StderrLogger(LogLevel min) { _min = min; }
    public IDisposable? BeginScope<TState>(TState state) where TState : notnull => null;
    public bool IsEnabled(LogLevel logLevel) => logLevel >= _min;
    public void Log<TState>(LogLevel logLevel, EventId eventId, TState state, Exception? exception, Func<TState, Exception?, string> formatter)
    {
        if (!IsEnabled(logLevel)) return;
        Console.Error.WriteLine($"[pab-host {logLevel}] {typeof(T).Name}: {formatter(state, exception)}{(exception != null ? " " + exception.Message : "")}");
    }
}

static class Program
{
    static Stream Out = null!;

    static void Send(byte[] json)
    {
        Span<byte> len = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(len, (uint)json.Length);
        Out.Write(len);
        Out.Write(json);
        Out.Flush();
    }

    static void SendObject(Action<Utf8JsonWriter> body)
    {
        using var ms = new MemoryStream();
        using (var w = new Utf8JsonWriter(ms))
        {
            w.WriteStartObject();
            body(w);
            w.WriteEndObject();
        }
        Send(ms.ToArray());
    }

    [System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr GetStdHandle(int nStdHandle);

    /// A FileStream over the standard handle `which` (-10 in, -11 out) on Windows.
    static Stream? StdStream(int which, FileAccess access)
    {
        if (!OperatingSystem.IsWindows()) return null;
        var h = GetStdHandle(which);
        if (h == IntPtr.Zero || h == new IntPtr(-1)) return null;
        try
        {
            return new FileStream(new Microsoft.Win32.SafeHandles.SafeFileHandle(h, false), access, 1);
        }
        catch (Exception)
        {
            return null;
        }
    }

    static void ReadExact(Stream s, Span<byte> into)
    {
        while (into.Length > 0)
        {
            int n = s.Read(into);
            if (n <= 0) throw new EndOfStreamException();
            into = into[n..];
        }
    }

    static uint ReadU32(Stream s)
    {
        Span<byte> b = stackalloc byte[4];
        ReadExact(s, b);
        return BinaryPrimitives.ReadUInt32LittleEndian(b);
    }

    static int Main(string[] args)
    {
        string? mappers = null, mapperPath = null;
        var minLevel = LogLevel.Warning;
        bool compact = false;
        bool trace = Environment.GetEnvironmentVariable("PAB_TRACE") == "1";
        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--mappers": mappers = args[++i]; break;
                case "--mapper": mapperPath = args[++i]; break;
                case "--verbose": minLevel = LogLevel.Information; break;
                case "--compact": compact = true; break;
            }
        }
        if (mappers == null || mapperPath == null)
        {
            Console.Error.WriteLine("usage: pab-host --mappers <folder> --mapper <relative .xml path> [--verbose]");
            return 2;
        }

        // Poke-A-Byte prints some exceptions with Console.WriteLine: keep them off the
        // protocol stream.
        Console.SetOut(Console.Error);
        // Console streams write a pipe with one WriteFile call and do not finish a partial
        // write, which loses the end of a long answer when the reader is slow; FileStreams over
        // the same handles loop until everything is written.
        Out = StdStream(-11, FileAccess.Write) ?? Console.OpenStandardOutput();
        var input = new BufferedStream(StdStream(-10, FileAccess.Read) ?? Console.OpenStandardInput(), 1 << 20);

        var options = new JsonSerializerOptions(JsonSerializerDefaults.Web);
        options.Converters.Add(new ByteArrayJsonConverter());
        var notifier = new Notifier(options) { Compact = compact };
        var driver = new StdinDriver();

        MapperContent content;
        PokeAByteInstance instance;
        PokeAByteInstance NewInstance()
        {
            var created = new PokeAByteInstance(
                new StderrLogger<PokeAByteInstance>(minLevel),
                new ScriptConsole(new StderrLogger<ScriptConsole>(minLevel)),
                notifier,
                content,
                driver);
            if (!driver.Configured)
            {
                // The first read only tells the driver which blocks Poke-A-Byte reads.
                try { created.Read().GetAwaiter().GetResult(); }
                catch (ProbeException) { }
            }
            return created;
        }
        try
        {
            var xmlPath = Path.Combine(mappers, mapperPath);
            var jsPath = Path.ChangeExtension(xmlPath, ".js");
            content = new MapperContent(
                mapperPath.Replace('\\', '/'),
                File.ReadAllText(xmlPath),
                File.Exists(jsPath) ? Path.GetFullPath(jsPath) : null,
                File.Exists(jsPath) ? Path.GetFullPath(mappers) : null);
            instance = NewInstance();
        }
        catch (Exception ex)
        {
            SendObject(w => w.WriteString("error", $"could not load the mapper: {ex.Message}"));
            return 1;
        }

        var meta = instance.Mapper.Metadata;
        SendObject(w =>
        {
            w.WriteStartArray("blocks");
            for (int i = 0; i < driver.Blocks.Length; i++)
            {
                w.WriteStartArray();
                w.WriteNumberValue(driver.Starts[i]);
                w.WriteNumberValue(driver.Blocks[i].Length);
                w.WriteEndArray();
            }
            w.WriteEndArray();
            w.WriteString("gameName", meta.GameName);
            w.WriteString("platform", meta.GamePlatform);
        });

        bool started = false;
        var empty = Encoding.UTF8.GetBytes("[]");
        while (true)
        {
            int cmd;
            try { cmd = input.ReadByte(); }
            catch (IOException) { return 0; }
            if (cmd < 0 || cmd == 'Q') return 0;
            try
            {
                switch (cmd)
                {
                    case 'S':
                    {
                        uint runs = ReadU32(input);
                        if (trace) Console.Error.WriteLine($"[trace] S with {runs} runs");
                        for (uint r = 0; r < runs; r++)
                        {
                            uint block = ReadU32(input), offset = ReadU32(input), length = ReadU32(input);
                            if (block >= driver.Blocks.Length || (ulong)offset + length > (ulong)driver.Blocks[block].Length)
                                throw new InvalidDataException($"run {block}:{offset}+{length} is outside the blocks");
                            ReadExact(input, driver.Blocks[block].AsSpan((int)offset, (int)length));
                        }
                        if (trace) Console.Error.WriteLine("[trace] runs read");
                        notifier.Changes = null;
                        notifier.Errors.Clear();
                        instance.Read().GetAwaiter().GetResult();
                        if (trace) Console.Error.WriteLine("[trace] read 1 done");
                        if (!started)
                        {
                            // StartProcessing reads twice before the mapper counts as loaded.
                            instance.Read().GetAwaiter().GetResult();
                            started = true;
                        }
                        if (notifier.Errors.Count > 0)
                        {
                            foreach (var e in notifier.Errors) Console.Error.WriteLine($"[pab-host] {e}");
                        }
                        Send(notifier.Changes ?? empty);
                        break;
                    }
                    case 'M':
                    {
                        using var ms = new MemoryStream();
                        using (var w = new Utf8JsonWriter(ms))
                        {
                            w.WriteStartObject();
                            w.WriteStartObject("meta");
                            w.WriteString("id", meta.Id);
                            w.WriteString("gameName", meta.GameName);
                            w.WriteString("gamePlatform", meta.GamePlatform);
                            w.WriteString("version", meta.Version);
                            w.WriteString("path", meta.Path);
                            w.WriteEndObject();
                            w.WritePropertyName("properties");
                            JsonSerializer.Serialize(w, instance.Mapper.Properties.Values.ToList(), options);
                            w.WriteStartObject("glossary");
                            foreach (var reference in instance.Mapper.References.Values)
                            {
                                w.WriteStartArray(reference.Name);
                                foreach (var item in reference.Values)
                                {
                                    w.WriteStartObject();
                                    w.WriteNumber("key", item.Key);
                                    w.WritePropertyName("value");
                                    JsonSerializer.Serialize(w, item.Value, options);
                                    w.WriteEndObject();
                                }
                                w.WriteEndArray();
                            }
                            w.WriteEndObject();
                            w.WriteEndObject();
                        }
                        Send(ms.ToArray());
                        break;
                    }
                    case 'X':
                    {
                        Send(ScriptState.Export(instance, options));
                        break;
                    }
                    case 'I':
                    {
                        uint len = ReadU32(input);
                        var buf = new byte[len];
                        ReadExact(input, buf);
                        int n = ScriptState.Import(instance, buf);
                        SendObject(w => w.WriteNumber("imported", n));
                        break;
                    }
                    case 'R':
                    {
                        instance.DisposeAsync().AsTask().GetAwaiter().GetResult();
                        foreach (var b in driver.Blocks) Array.Clear(b);
                        instance = NewInstance();
                        meta = instance.Mapper.Metadata;
                        started = false;
                        SendObject(w => w.WriteBoolean("reset", true));
                        break;
                    }
                    default:
                        throw new InvalidDataException($"unknown command {cmd}");
                }
            }
            catch (EndOfStreamException)
            {
                return 0;
            }
            catch (Exception ex)
            {
                var msg = ex.InnerException != null ? $"{ex.Message} ({ex.InnerException.Message})" : ex.Message;
                SendObject(w => w.WriteString("error", msg));
            }
        }
    }
}
