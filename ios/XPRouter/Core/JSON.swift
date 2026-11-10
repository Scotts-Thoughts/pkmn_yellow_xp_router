import Foundation

/// A JSON value from the Rust core. The core hands back view models as
/// JSON; views read them through subscripts (`row["name"].string`) rather
/// than one Codable struct per screen, which keeps the Swift side a thin
/// renderer of what the shared logic computed.
@dynamicMemberLookup
enum JSON: Hashable, Sendable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    init(any: Any?) {
        switch any {
        case nil, is NSNull: self = .null
        case let b as Bool where type(of: b) == Bool.self: self = .bool(b)
        case let n as NSNumber:
            if CFGetTypeID(n) == CFBooleanGetTypeID() { self = .bool(n.boolValue) } else { self = .number(n.doubleValue) }
        case let s as String: self = .string(s)
        case let a as [Any]: self = .array(a.map { JSON(any: $0) })
        case let o as [String: Any]: self = .object(o.mapValues { JSON(any: $0) })
        case let j as JSON: self = j
        default: self = .null
        }
    }

    static func parse(_ data: Data) -> JSON {
        guard let any = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) else { return .null }
        return JSON(any: any)
    }

    var any: Any {
        switch self {
        case .null: return NSNull()
        case .bool(let b): return b
        case .number(let n): return n
        case .string(let s): return s
        case .array(let a): return a.map { $0.any }
        case .object(let o): return o.mapValues { $0.any }
        }
    }

    subscript(key: String) -> JSON {
        if case .object(let o) = self { return o[key] ?? .null }
        return .null
    }

    subscript(index: Int) -> JSON {
        if case .array(let a) = self, index >= 0, index < a.count { return a[index] }
        return .null
    }

    subscript(dynamicMember key: String) -> JSON { self[key] }

    var isNull: Bool { if case .null = self { return true } else { return false } }
    var string: String? {
        switch self {
        case .string(let s): return s
        case .number(let n): return n == n.rounded() ? String(Int(n)) : String(n)
        default: return nil
        }
    }
    var str: String { string ?? "" }
    var int: Int? {
        switch self {
        case .number(let n): return Int(n)
        case .string(let s): return Int(s)
        case .bool(let b): return b ? 1 : 0
        default: return nil
        }
    }
    var double: Double? { if case .number(let n) = self { return n }; return nil }
    var bool: Bool { if case .bool(let b) = self { return b }; return false }
    var array: [JSON] { if case .array(let a) = self { return a }; return [] }
    var object: [String: JSON] { if case .object(let o) = self { return o }; return [:] }
    var strings: [String] { array.compactMap { $0.string } }
    var ints: [Int] { array.compactMap { $0.int } }
}

extension JSON: ExpressibleByStringLiteral, ExpressibleByIntegerLiteral, ExpressibleByBooleanLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral, ExpressibleByNilLiteral, ExpressibleByFloatLiteral {
    init(stringLiteral value: String) { self = .string(value) }
    init(integerLiteral value: Int) { self = .number(Double(value)) }
    init(floatLiteral value: Double) { self = .number(value) }
    init(booleanLiteral value: Bool) { self = .bool(value) }
    init(arrayLiteral elements: JSON...) { self = .array(elements) }
    init(dictionaryLiteral elements: (String, JSON)...) { self = .object(Dictionary(elements, uniquingKeysWith: { a, _ in a })) }
    init(nilLiteral: ()) { self = .null }
}

extension JSON {
    init(_ s: String?) { self = s.map { .string($0) } ?? .null }
    init(_ i: Int?) { self = i.map { .number(Double($0)) } ?? .null }
    init(_ b: Bool) { self = .bool(b) }
    init(_ a: [String]) { self = .array(a.map { .string($0) }) }
    init(_ a: [Int]) { self = .array(a.map { .number(Double($0)) }) }
}

/// A row-like JSON with a stable id, for ForEach.
struct Item: Identifiable, Hashable {
    let json: JSON
    let id: String
    init(_ json: JSON, key: String = "id") {
        self.json = json
        self.id = json[key].string ?? UUID().uuidString
    }
    subscript(key: String) -> JSON { json[key] }
}
