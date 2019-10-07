# Oasis RPC Format

This doc describes the message format expected by services generated using oasis-rs.
This is the format used by [oasis.js](https://github.com/oasislabs/oasis.js) when invoking methods on service objects loaded from an IDL.

The types are defined in the [oasis-rpc](/oasis-rpc/src/lib.rs) crate and the wire format is [CBOR](https://tools.ietf.org/html/rfc7049).
You can find an example of the interface definition language (IDL) in the [tests](/tests/idl-gen/res/TestService.json).

Note: If you find that CBOR is not efficient enough, or don't need struct-as-state semantics, you can build your service using `oasis build --wasi`, which will skip generating an interface or method dispatch tree.
Of course, you'll also need a client that speaks your protocol's language.
So, in the words of The Architect, you are allowed the power to choose, even if only aware of the choice on a vague, subconscious level.

## Types

### Primitive types

The primitive types are defined in the `Type` enum in oasis-rpc.
They're basically what you expect from any reasonable RPC type system, except for maybe a few.
These few are: `Address`, `Tuple`, `Array`, `List`, `Optional`, and `Result`.

You probably already grok the difference between `Array` and `List`, but just to be clear, an `Array` has a fixed length forever and always, whereas a `List` can be extended by the receiver.
You can pass an `Array` where a `List` is expected and also the reverse as long as the `List` has the correct length.

`Tuple` is like an `Array`, but with (optionally) heterogeneous types.

`Address` and `Balance` are the first Oasis-specific types and correspond to 160-bit account addresses, and 128-bit account balances.
Client implementations may expose the byte contents of `Address`, but not `Balance` since the latter depends on endian-ness.

`Optional<T>` is just a long way of saying that the type `T` is _nullable_ and corresponds to `null` or `Maybe` in other languages.

`Result<T, E>` is generally used by functions that might return an error.
It's like `Either` in Haskell-inspired languages.
In the CBOR format, this looks like `{ "Ok": value }` or `{ "Err": value }`.
So, in other words, it's encoded as an enum with `Ok` and `Err` variants (but we'll get to that in a bit).

### Defined types

You can define your own types (of course).
`struct` and `enum` are fully supported.
Additonally, structs can be used as `Event`s, which can be picked up by off-chain clients.
Up to three of the struct's fields can be marked as `indexed`, which allows off-chain listeners to efficiently filter for subscribed events.

Defined types are recorded in the interface's `type_defs` field; only those used in an RPC method are exported, however.
Defined types from other RPC interfaces will be linked to in the interface's `imports` section.

**Note**: you can only expose types in an RPC interface that were (transitively) also defined in an RPC interface.
This is to say that you can't just return a type from a crate--even if it's serializable.
For instance, I might want to use a big integer crate that contains a `[u8; 32]` type, but I'd have to re-define it in my own crate to export it in an RPC.
The reason for this is that services written in one language can't directly use the libraries from another language.
The quick solution is to re-define the type in your own interface, but if you find this annoying, please upvote [oasislabs/oasis-rs#213](https://github.com/oasislabs/oasis-rs/issues/213), so that we know to prioritize automating this step.

### Functions

Functions can be either methods on an deployed service or the `Constructor`.
A `Constructor` is an anonymous function that takes inputs and, optionally, returns an error; it does not return anything because the output is the service that's persisted to the blockchain.
Otherwise, a `Function` is named, has named arguments (`Field`s), and can return whatever (as long as it's an RPC `Type`).
A `Function` can be marked as mutable or immutable, but this isn't enforced by the platform; it's just to help the author of the service write safer code.

## Wire format

Messages are structured as

```json
{
  "method": "<method name>",
  "payload": [args, "..."],
}
```

`method` identifies the name of the RPC to be called and `payload` is a (possibly empty) list of positional arguments.
Keyword arguments are not supported since language support is far from ubiquitious; the IDL contains the names only for ease of debugging generated clients.

The wire format for an argument is the canonical CBOR encoding of the argument's type.
Also worth mentioning is that `Address` is serialized as a byte array and `Balance` is a CBOR bigint.

Struct variants are represented as objects containing the field names as keys.
Tuple variants are represented as arrays containing the positional values.
Enums with payloads (i.e. tagged unions) look like `"VariantName": <data>`, else they're just strings `"VariantName"`.
