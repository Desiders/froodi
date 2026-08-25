//! Scale tests for `registry!`. Every registry here is built under rustc's *default*
//! `recursion_limit` -- the absence of a `#![recursion_limit = "..."]` attribute is the point.
#![no_std]

extern crate alloc;

use froodi::{registry, Container, DefaultScope::App, InstantiateErrorKind};

#[cfg(feature = "async")]
use froodi::async_registry;

pub struct T<const N: usize>;

fn inst<const N: usize>() -> Result<T<N>, InstantiateErrorKind> {
    Ok(T::<N>)
}

/// Feeds `0 1 .. 499` to `$mac`, so the 500 clauses below stay one flat repetition.
macro_rules! with_500_ids {
    ($mac:ident) => {
        $mac! {
        0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38
        39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63 64 65 66 67 68 69 70 71 72 73 74
        75 76 77 78 79 80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95 96 97 98 99 100 101 102 103 104 105 106 107
        108 109 110 111 112 113 114 115 116 117 118 119 120 121 122 123 124 125 126 127 128 129 130 131 132 133 134
        135 136 137 138 139 140 141 142 143 144 145 146 147 148 149 150 151 152 153 154 155 156 157 158 159 160 161
        162 163 164 165 166 167 168 169 170 171 172 173 174 175 176 177 178 179 180 181 182 183 184 185 186 187 188
        189 190 191 192 193 194 195 196 197 198 199 200 201 202 203 204 205 206 207 208 209 210 211 212 213 214 215
        216 217 218 219 220 221 222 223 224 225 226 227 228 229 230 231 232 233 234 235 236 237 238 239 240 241 242
        243 244 245 246 247 248 249 250 251 252 253 254 255 256 257 258 259 260 261 262 263 264 265 266 267 268 269
        270 271 272 273 274 275 276 277 278 279 280 281 282 283 284 285 286 287 288 289 290 291 292 293 294 295 296
        297 298 299 300 301 302 303 304 305 306 307 308 309 310 311 312 313 314 315 316 317 318 319 320 321 322 323
        324 325 326 327 328 329 330 331 332 333 334 335 336 337 338 339 340 341 342 343 344 345 346 347 348 349 350
        351 352 353 354 355 356 357 358 359 360 361 362 363 364 365 366 367 368 369 370 371 372 373 374 375 376 377
        378 379 380 381 382 383 384 385 386 387 388 389 390 391 392 393 394 395 396 397 398 399 400 401 402 403 404
        405 406 407 408 409 410 411 412 413 414 415 416 417 418 419 420 421 422 423 424 425 426 427 428 429 430 431
        432 433 434 435 436 437 438 439 440 441 442 443 444 445 446 447 448 449 450 451 452 453 454 455 456 457 458
        459 460 461 462 463 464 465 466 467 468 469 470 471 472 473 474 475 476 477 478 479 480 481 482 483 484 485
        486 487 488 489 490 491 492 493 494 495 496 497 498 499
        }
    };
}

macro_rules! entries_in_one_scope {
    ($($i:literal)+) => {
        registry! { scope(App) [ $( provide(inst::<$i>) ),+ ] }
    };
}

macro_rules! scope_clauses {
    ($($i:literal)+) => {
        registry! { $( scope(App) [ provide(inst::<$i>) ] ),+ }
    };
}

macro_rules! provide_clauses {
    ($($i:literal)+) => {
        registry! { $( provide(App, inst::<$i>) ),+ }
    };
}

macro_rules! provide_clauses_then_extend {
    ($($i:literal)+) => {
        registry! {
            $( provide(App, inst::<$i>) ),+,
            extend(registry! { scope(App) [ provide(|| Ok(Marker)) ] }),
        }
    };
}

struct Marker;

#[test]
fn n500_provides_in_a_single_scope() {
    let container = Container::new(with_500_ids!(entries_in_one_scope));

    assert!(container.get::<T<0>>().is_ok());
    assert!(container.get::<T<250>>().is_ok());
    assert!(container.get::<T<499>>().is_ok());
}

#[test]
fn n500_scope_clauses() {
    let container = Container::new(with_500_ids!(scope_clauses));

    assert!(container.get::<T<0>>().is_ok());
    assert!(container.get::<T<250>>().is_ok());
    assert!(container.get::<T<499>>().is_ok());
}

#[test]
fn n500_provide_clauses() {
    let container = Container::new(with_500_ids!(provide_clauses));

    assert!(container.get::<T<0>>().is_ok());
    assert!(container.get::<T<250>>().is_ok());
    assert!(container.get::<T<499>>().is_ok());
}

#[test]
fn n500_clauses_with_a_trailing_extend() {
    let container = Container::new(with_500_ids!(provide_clauses_then_extend));

    assert!(container.get::<T<0>>().is_ok());
    assert!(container.get::<T<499>>().is_ok());
    assert!(container.get::<Marker>().is_ok());
}

/// `extend(registry!(...))` is lexical nesting, so its depth is bounded by `recursion_limit`
/// however the clause list is matched. 20 levels is far past what composing modules needs.
macro_rules! nest {
    ($i:literal) => {
        registry! { scope(App) [ provide(inst::<$i>) ] }
    };
    ($i:literal $($rest:literal)+) => {
        registry! { provide(App, inst::<$i>), extend(nest!($($rest)+)) }
    };
}

#[test]
fn nested_extend_20_levels() {
    let container = Container::new(nest!(19 18 17 16 15 14 13 12 11 10 9 8 7 6 5 4 3 2 1 0));

    assert!(container.get::<T<0>>().is_ok());
    assert!(container.get::<T<19>>().is_ok());
}

#[cfg(feature = "async")]
async fn async_inst<const N: usize>() -> Result<T<N>, InstantiateErrorKind> {
    Ok(T::<N>)
}

#[cfg(feature = "async")]
macro_rules! async_entries_in_one_scope {
    ($($i:literal)+) => {
        async_registry! { scope(App) [ $( provide(async_inst::<$i>) ),+ ] }
    };
}

/// `async_registry!` shares the clause-list arm with `registry!`; building the registry needs no
/// runtime, so this stays a plain test.
#[cfg(feature = "async")]
#[test]
fn n500_async_provides_in_a_single_scope() {
    let registry = with_500_ids!(async_entries_in_one_scope);

    registry.validate().unwrap();
}
