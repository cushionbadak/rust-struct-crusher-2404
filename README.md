# Rust Struct Crusher

It crushes struct definiton of the given rust file

### Goal Examples
```Rust
struct S;     // Input 1  - unit struct
struct S();   // Output 1 - crush it to empty-tuple struct

struct DropMe(&'static str); // Input 2  - nonempty-tuple struct
struct DropMe();            // Output 2 - empty-tuple struct

struct S { t: i32 }   // Input 3  - struct struct
struct S;             // Ouptut 3 - unit struct
```

If multiple struct definition exists in one file, crushes one-by-one, and save it as separate file.

```Rust
// Input File "asdf.rs"
struct S;
struct DropMe(& 'static str);


// Output File 1 "asdf_1.rs"
struct S();
struct DropMe(& 'static str);


// Output File 2 "asdf_2.rs"
struct S;
struct DropMe();
```
