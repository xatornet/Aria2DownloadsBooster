using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace AriaXtr.Services;

public class AriaRpcException : Exception
{
    public string FaultCode { get; }

    public AriaRpcException(string code, string message) : base(message)
    {
        FaultCode = code;
    }

    /// <summary>El download ya no existe en el demonio aria2 (p.ej. tras reiniciar).
    /// aria2 responde literalmente "GID &lt;hex&gt; is not found" (¡no "No such"!).</summary>
    public bool IsUnknownGid =>
        Message.Contains("is not found", StringComparison.OrdinalIgnoreCase) ||
        Message.Contains("No such", StringComparison.OrdinalIgnoreCase) ||
        FaultCode == "RESOURCE_NOT_FOUND";
}

/// <summary>Formato de los elementos de system.multicall en JSON-RPC.</summary>
public static class MulticallResult
{
    /// <summary>
    /// JSON-RPC multicall devuelve por cada sub-llamada: [valor] si OK (¡array directo, no un
    /// objeto con "result"!), o un objeto {"code":…,"message":…} si error.
    /// TryGet tolera además la forma envolvente {"result":[valor]} por si acaso.
    /// </summary>
    public static bool TryGet(JsonElement el, out JsonElement value)
    {
        value = default;
        if (el.ValueKind == JsonValueKind.Array && el.GetArrayLength() > 0)
        {
            value = el[0];
            return true;
        }
        if (el.ValueKind == JsonValueKind.Object
            && el.TryGetProperty("result", out var arr)
            && arr.ValueKind == JsonValueKind.Array && arr.GetArrayLength() > 0)
        {
            value = arr[0];
            return true;
        }
        return false;
    }

    public static string? GetError(JsonElement el)
    {
        if (el.ValueKind != JsonValueKind.Object) return null;
        if (el.TryGetProperty("message", out var m)) return m.GetString();
        if (el.TryGetProperty("error", out var e) && e.TryGetProperty("message", out var m2))
            return m2.GetString();
        return null;
    }
}

/// <summary>Cliente JSON-RPC mínimo para el demonio aria2c.</summary>
public sealed class AriaRpc
{
    private readonly HttpClient _http;
    private readonly string _url;
    private readonly string _secret;

    public AriaRpc(int port, string secret)
    {
        _url = $"http://127.0.0.1:{port}/jsonrpc";
        _secret = secret;
        _http = new HttpClient { Timeout = TimeSpan.FromSeconds(12) };
    }

    public async Task<JsonElement> CallAsync(string method, IEnumerable<object?>? extraParams = null,
        CancellationToken ct = default)
    {
        var prms = new List<object?> { "token:" + _secret };
        if (extraParams != null) prms.AddRange(extraParams);
        return await CallRawAsync(method, prms, ct);
    }

    /// <summary>Llamada con params tal cual (sin token añadido) — usada por MulticallAsync.</summary>
    private async Task<JsonElement> CallRawAsync(string method, List<object?> prms, CancellationToken ct)
    {
        var payload = JsonSerializer.Serialize(new Dictionary<string, object?>
        {
            ["jsonrpc"] = "2.0",
            ["id"] = "1",
            ["method"] = method,
            ["params"] = prms,
        });

        using var content = new StringContent(payload, Encoding.UTF8, "application/json");
        using var resp = await _http.PostAsync(_url, content, ct);
        var text = await resp.Content.ReadAsStringAsync(ct);

        using var doc = JsonDocument.Parse(text);
        if (doc.RootElement.TryGetProperty("error", out var err))
        {
            var code = err.TryGetProperty("code", out var c) ? c.ToString() : "";
            var msg = err.TryGetProperty("message", out var m) ? m.GetString() : "aria2 error";
            throw new AriaRpcException(code, msg ?? "aria2 error");
        }
        return doc.RootElement.GetProperty("result").Clone();
    }

    /// <summary>
    /// system.multicall compatible con --rpc-secret: aria2 NO admite token en el nivel exterior
    /// de multicall (params[0] debe ser la lista de MethodCall) y exige "token:xxx" como primer
    /// argumento de CADA sub-llamada. Cada elemento devuelto es {"result":[...]} o {"error":{...}}.
    /// </summary>
    public Task<JsonElement> MulticallAsync(IEnumerable<(string Method, object?[] Args)> calls,
        CancellationToken ct = default)
    {
        var specs = calls.Select(c =>
        {
            var sub = new List<object?> { "token:" + _secret };
            sub.AddRange(c.Args);
            return new Dictionary<string, object?>
            {
                ["methodName"] = c.Method,
                ["params"] = sub,
            };
        }).ToList();

        return CallRawAsync("system.multicall", new List<object?> { specs }, ct);
    }

    /// <summary>Atajo para un único argumento string (gid…).</summary>
    public Task<JsonElement> CallAsync(string method, string arg, CancellationToken ct = default)
        => CallAsync(method, new object?[] { arg }, ct);

    /// <summary>Atajo para un único argumento diccionario (opciones aria2).</summary>
    public Task<JsonElement> CallAsync(string method, Dictionary<string, string> single,
        CancellationToken ct = default)
        => CallAsync(method, new object?[] { single }, ct);

    /// <summary>Versión de aria2 sin necesidad de token (para probes de arranque).</summary>
    public async Task<string?> TryGetVersionAsync()
    {
        try
        {
            var r = await CallAsync("aria2.getVersion");
            return r.TryGetProperty("version", out var v) ? v.GetString() : "?";
        }
        catch
        {
            return null;
        }
    }
}
