## Math
# Math

This is the first blog post. It keeps *small emphasis*, **important points**,
~~unnecessary ceremony~~, and `inline code` close to the prose.

Inline mathematics, such as $a^2 + b^2 = c^2$, can stay close to the sentence
that needs it. A second display formula has room to breathe:

$$
\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}
$$

## Audio

The short spoken introduction below is a local audio asset. Its relative URL
is resolved by the browser from this post's generated URL, so it remains
correct when the site is served below its configured base path. It is long
enough to try seeking while `heine serve` handles byte-range requests.[^audio]

<audio controls preload="metadata">
  <source src="audio/heine-introduction.mp3" type="audio/mpeg">
  Your browser does not support the audio element.
</audio>

## Code

```rust
fn main() {
    println!("Hello, world!");
}
```

## Working Notes

The same Markdown source can keep a few ordinary structural notes nearby:

- formulas remain readable before they become MathML;
- media stays beside the page that uses it;
- checked page links, such as the [blog index](page:blog/index), remain
  ordinary links in the source:
  - the authored relationship is easy to read;
  - the generated URL is resolved by Heine.

The short route from source to result is equally suitable for a numbered
sequence:

1. write the page;
2. build it;
3. listen to the result.

And for a modest checklist:

- [x] Keep the formula in the article.
- [x] Keep the audio asset with the article.
- [ ] Convince Euclid to record a sequel.

| Source form     | Here it provides               |
|-----------------|--------------------------------|
| `*text*`        | a small emphasis               |
| `` `code` ``    | an identifier in running prose |
| a note[^source] | a useful aside                 |

## Right-to-Left Text

<blockquote lang="ar" dir="rtl">
  <p>لا يوجد طريقٌ ملوكيٌّ إلى الهندسة.</p>
</blockquote>

Arabic rendering of "There is no royal road to geometry", a traditional saying
attributed to Euclid by Proclus.

[^audio]: The audio element is trusted raw HTML because its controls and
    preload policy are browser features, not Markdown syntax.

[^source]: Footnotes keep an aside out of the sentence that prompted it.
