// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Format-agnostic deserialization recursion-depth guard for the AST (`serde`
//! feature).
//!
//! # Why this exists
//!
//! The parser bounds recursive descent at parse time
//! (`DEFAULT_RECURSION_LIMIT`), and everything downstream —
//! `Drop`, `Display`/render, and the generated `Visit` walks — is unbounded
//! self-recursion that today relies on that parse cap for its stack safety.
//! `Deserialize` bypasses the parser entirely, so untrusted serialized bytes can
//! reconstruct an arbitrarily deep tree that overflows the stack on its very first
//! `drop`/`to_string`/`visit`. The only robust defence is to reject the deep input
//! *before* the tree is built, i.e. during deserialization.
//!
//! `serde_json` already caps nesting depth itself (128 by default), so JSON — the
//! format the wasm/python bridges use — is guarded out of the box. That guard is
//! JSON-only, though; a non-self-describing format (bincode, postcard, …) has no
//! such limit. [`DepthLimited`](crate::serde_depth::DepthLimited) adds an equivalent, format-independent cap by
//! wrapping any `Deserializer` and threading a descent budget through every
//! nested value, so a tree deeper than the budget fails cleanly on *any* format.
//!
//! # Usage
//!
//! Wrap the format's deserializer before handing it to `T::deserialize`, or use
//! the [`from_deserializer`](crate::serde_depth::from_deserializer) convenience:
//!
//! ```
//! # #[cfg(feature = "serde")] {
//! use squonk_ast::serde_depth::{from_deserializer, DEFAULT_DESERIALIZE_DEPTH};
//! use squonk_ast::Span;
//!
//! let mut json = serde_json::Deserializer::from_str("{\"start\":1,\"end\":4}");
//! let span: Span = from_deserializer(&mut json, DEFAULT_DESERIALIZE_DEPTH).unwrap();
//! assert_eq!(span.start(), 1);
//! # }
//! ```
//!
//! The parse root (`Parsed`, in the `squonk` crate) deserializes its statement
//! tree through this wrapper automatically, so the primary public deserialization
//! path is depth-safe on every format without the caller doing anything.

use serde::de::{
    self, Deserialize, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};

/// Default deserialization nesting budget, analogous to the parser's
/// `DEFAULT_RECURSION_LIMIT`.
///
/// This counts *serde* nesting depth (each struct/enum/seq/map descent), which is a
/// small constant multiple of AST-node depth, not AST-node depth itself. The value
/// is deliberately conservative: deep enough for ordinary trees, shallow enough
/// that the deserialization recursion itself — which is as deep as the tree — never
/// approaches the stack limit. Callers round-tripping legitimately deep trees can
/// raise it via [`DepthLimited::new`] at their own stack-safety risk.
pub const DEFAULT_DESERIALIZE_DEPTH: usize = 128;

fn too_deep<E: de::Error>() -> E {
    E::custom("deserialization exceeded the configured recursion-depth limit")
}

/// A `Deserializer` adapter that rejects input nested deeper than a fixed budget.
///
/// Each descent into a compound value (struct, enum, seq, tuple, map) spends one
/// unit of budget; reaching zero and attempting to descend again fails with a
/// custom serde error rather than recursing further. The budget is threaded by
/// value, so it measures the longest root-to-leaf nesting chain, not the total node
/// count — sibling subtrees do not accumulate against each other.
pub struct DepthLimited<D> {
    inner: D,
    budget: usize,
}

impl<D> DepthLimited<D> {
    /// Wrap `inner`, allowing at most `max_depth` levels of nested compound values.
    pub fn new(inner: D, max_depth: usize) -> Self {
        Self {
            inner,
            budget: max_depth,
        }
    }
}

/// Deserialize a `T` from `deserializer`, bounding nesting depth to `max_depth`.
///
/// The ergonomic entry point over [`DepthLimited`]: it wraps `deserializer` and
/// drives `T::deserialize`, so untrusted input on any format is rejected before a
/// hostile-deep tree can be constructed (and later overflow the stack on drop).
///
/// # Errors
///
/// Returns the format's deserialization error, including a custom
/// depth-limit-exceeded error when the input nests past `max_depth`.
pub fn from_deserializer<'de, T, D>(deserializer: D, max_depth: usize) -> Result<T, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(DepthLimited::new(deserializer, max_depth))
}

/// Forward a scalar `deserialize_*` method unchanged: scalars are terminal, so they
/// neither spend budget nor need their visitor wrapped.
macro_rules! forward_scalar {
    ($($method:ident),* $(,)?) => {
        $(
            fn $method<V>(self, visitor: V) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.inner.$method(visitor)
            }
        )*
    };
}

impl<'de, D> Deserializer<'de> for DepthLimited<D>
where
    D: Deserializer<'de>,
{
    type Error = D::Error;

    forward_scalar! {
        deserialize_bool,
        deserialize_i8, deserialize_i16, deserialize_i32, deserialize_i64, deserialize_i128,
        deserialize_u8, deserialize_u16, deserialize_u32, deserialize_u64, deserialize_u128,
        deserialize_f32, deserialize_f64,
        deserialize_char, deserialize_str, deserialize_string,
        deserialize_bytes, deserialize_byte_buf,
        deserialize_unit, deserialize_identifier,
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        self.inner.deserialize_unit_struct(name, visitor)
    }

    // `Option` and newtype wrappers are not real nesting levels for AST depth: the
    // meaningful child is whatever they contain, which spends budget on its own
    // descent. Thread the budget through unchanged.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget;
        self.inner.deserialize_option(DepthVisitor {
            inner: visitor,
            budget,
        })
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget;
        self.inner.deserialize_newtype_struct(
            name,
            DepthVisitor {
                inner: visitor,
                budget,
            },
        )
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget.checked_sub(1).ok_or_else(too_deep)?;
        self.inner.deserialize_seq(DepthVisitor {
            inner: visitor,
            budget,
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget.checked_sub(1).ok_or_else(too_deep)?;
        self.inner.deserialize_tuple(
            len,
            DepthVisitor {
                inner: visitor,
                budget,
            },
        )
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget.checked_sub(1).ok_or_else(too_deep)?;
        self.inner.deserialize_tuple_struct(
            name,
            len,
            DepthVisitor {
                inner: visitor,
                budget,
            },
        )
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget.checked_sub(1).ok_or_else(too_deep)?;
        self.inner.deserialize_map(DepthVisitor {
            inner: visitor,
            budget,
        })
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget.checked_sub(1).ok_or_else(too_deep)?;
        self.inner.deserialize_struct(
            name,
            fields,
            DepthVisitor {
                inner: visitor,
                budget,
            },
        )
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget.checked_sub(1).ok_or_else(too_deep)?;
        self.inner.deserialize_enum(
            name,
            variants,
            DepthVisitor {
                inner: visitor,
                budget,
            },
        )
    }

    // Self-describing formats route unknown-shape and skipped values here; treat
    // them as potentially compound so a deep untyped/ignored subtree is bounded too.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget.checked_sub(1).ok_or_else(too_deep)?;
        self.inner.deserialize_any(DepthVisitor {
            inner: visitor,
            budget,
        })
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, D::Error>
    where
        V: Visitor<'de>,
    {
        let budget = self.budget.checked_sub(1).ok_or_else(too_deep)?;
        self.inner.deserialize_ignored_any(DepthVisitor {
            inner: visitor,
            budget,
        })
    }

    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

/// Visitor wrapper that re-wraps the access objects and child deserializers a format
/// hands back, so every element is deserialized through a [`DepthLimited`] carrying
/// the (already-decremented) child budget.
struct DepthVisitor<V> {
    inner: V,
    budget: usize,
}

impl<'de, V> Visitor<'de> for DepthVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.inner.expecting(formatter)
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        self.inner.visit_bool(v)
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        self.inner.visit_i64(v)
    }
    fn visit_i128<E: de::Error>(self, v: i128) -> Result<Self::Value, E> {
        self.inner.visit_i128(v)
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        self.inner.visit_u64(v)
    }
    fn visit_u128<E: de::Error>(self, v: u128) -> Result<Self::Value, E> {
        self.inner.visit_u128(v)
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        self.inner.visit_f64(v)
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        self.inner.visit_str(v)
    }
    fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
        self.inner.visit_borrowed_str(v)
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        self.inner.visit_string(v)
    }
    fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
        self.inner.visit_bytes(v)
    }
    fn visit_borrowed_bytes<E: de::Error>(self, v: &'de [u8]) -> Result<Self::Value, E> {
        self.inner.visit_borrowed_bytes(v)
    }
    fn visit_byte_buf<E: de::Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
        self.inner.visit_byte_buf(v)
    }
    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        self.inner.visit_none()
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        self.inner.visit_unit()
    }

    fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
    where
        D2: Deserializer<'de>,
    {
        self.inner.visit_some(DepthLimited {
            inner: deserializer,
            budget: self.budget,
        })
    }

    fn visit_newtype_struct<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
    where
        D2: Deserializer<'de>,
    {
        self.inner.visit_newtype_struct(DepthLimited {
            inner: deserializer,
            budget: self.budget,
        })
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        self.inner.visit_seq(DepthSeqAccess {
            inner: seq,
            budget: self.budget,
        })
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        self.inner.visit_map(DepthMapAccess {
            inner: map,
            budget: self.budget,
        })
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        self.inner.visit_enum(DepthEnumAccess {
            inner: data,
            budget: self.budget,
        })
    }
}

struct DepthSeqAccess<A> {
    inner: A,
    budget: usize,
}

impl<'de, A> SeqAccess<'de> for DepthSeqAccess<A>
where
    A: SeqAccess<'de>,
{
    type Error = A::Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, A::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.inner.next_element_seed(DepthSeed {
            inner: seed,
            budget: self.budget,
        })
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

struct DepthMapAccess<A> {
    inner: A,
    budget: usize,
}

impl<'de, A> MapAccess<'de> for DepthMapAccess<A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, A::Error>
    where
        K: DeserializeSeed<'de>,
    {
        self.inner.next_key_seed(DepthSeed {
            inner: seed,
            budget: self.budget,
        })
    }

    fn next_value_seed<Vv>(&mut self, seed: Vv) -> Result<Vv::Value, A::Error>
    where
        Vv: DeserializeSeed<'de>,
    {
        self.inner.next_value_seed(DepthSeed {
            inner: seed,
            budget: self.budget,
        })
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

struct DepthEnumAccess<A> {
    inner: A,
    budget: usize,
}

impl<'de, A> EnumAccess<'de> for DepthEnumAccess<A>
where
    A: EnumAccess<'de>,
{
    type Error = A::Error;
    type Variant = DepthVariantAccess<A::Variant>;

    fn variant_seed<Vs>(self, seed: Vs) -> Result<(Vs::Value, Self::Variant), A::Error>
    where
        Vs: DeserializeSeed<'de>,
    {
        let budget = self.budget;
        let (value, variant) = self.inner.variant_seed(DepthSeed {
            inner: seed,
            budget,
        })?;
        Ok((
            value,
            DepthVariantAccess {
                inner: variant,
                budget,
            },
        ))
    }
}

struct DepthVariantAccess<A> {
    inner: A,
    budget: usize,
}

impl<'de, A> VariantAccess<'de> for DepthVariantAccess<A>
where
    A: VariantAccess<'de>,
{
    type Error = A::Error;

    fn unit_variant(self) -> Result<(), A::Error> {
        self.inner.unit_variant()
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, A::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.inner.newtype_variant_seed(DepthSeed {
            inner: seed,
            budget: self.budget,
        })
    }

    fn tuple_variant<Vv>(self, len: usize, visitor: Vv) -> Result<Vv::Value, A::Error>
    where
        Vv: Visitor<'de>,
    {
        self.inner.tuple_variant(
            len,
            DepthVisitor {
                inner: visitor,
                budget: self.budget,
            },
        )
    }

    fn struct_variant<Vv>(
        self,
        fields: &'static [&'static str],
        visitor: Vv,
    ) -> Result<Vv::Value, A::Error>
    where
        Vv: Visitor<'de>,
    {
        self.inner.struct_variant(
            fields,
            DepthVisitor {
                inner: visitor,
                budget: self.budget,
            },
        )
    }
}

struct DepthSeed<S> {
    inner: S,
    budget: usize,
}

impl<'de, S> DeserializeSeed<'de> for DepthSeed<S>
where
    S: DeserializeSeed<'de>,
{
    type Value = S::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.inner.deserialize(DepthLimited {
            inner: deserializer,
            budget: self.budget,
        })
    }
}
