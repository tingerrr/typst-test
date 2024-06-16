#assert-panic(() => {
  panic()
})

#let (error,) = catch(() => {
  panic("hello there")
})

#assert.eq(error, "panicked with: \"hello there\"")
