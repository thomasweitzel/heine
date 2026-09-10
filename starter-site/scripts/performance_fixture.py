#!/usr/bin/env python3
"""Create or remove deterministic synthetic posts for local Heine profiling."""

from __future__ import annotations

import argparse
import random
import shutil
from datetime import date, timedelta
from pathlib import Path


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
SITE_DIRECTORY = SCRIPT_DIRECTORY.parent
POSTS_DIRECTORY = SITE_DIRECTORY / "content" / "en" / "performance-posts"
START_DATE = date(2020, 12, 31)
DEFAULT_COUNT = 10_000
DEFAULT_WORDS = 200
TAGS = (
    "Performance",
    "Static sites",
    "Rust",
    "Build systems",
    "Benchmarking",
    "Documentation",
    "Publishing",
    "Testing",
)
WORDS = (
    "a", "build", "checked", "content", "deterministic", "example", "generated",
    "Heine", "local", "page", "performance", "post", "rendered", "site", "static",
    "template", "test", "the", "this", "with",
)


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    generate = commands.add_parser("generate", help="replace the synthetic post subtree")
    generate.add_argument("--count", type=positive_integer, default=DEFAULT_COUNT)
    generate.add_argument("--words", type=positive_integer, default=DEFAULT_WORDS)
    generate.add_argument("--seed", type=int, default=0)

    commands.add_parser("clean", help="remove only the synthetic post subtree")
    return parser.parse_args()


def positive_integer(value: str) -> int:
    integer = int(value)
    if integer < 1:
        raise argparse.ArgumentTypeError("must be greater than zero")
    return integer


def reset_posts_directory() -> None:
    if POSTS_DIRECTORY.exists():
        shutil.rmtree(POSTS_DIRECTORY)
    POSTS_DIRECTORY.mkdir(parents=True)


def write_directory_descriptor() -> None:
    (POSTS_DIRECTORY / "_directory.toml").write_text(
        """[pages]
template = "post.tera"
collections = ["posts"]
authors = ["Performance fixture"]

[pages.metadata]
required = ["title", "published", "summary"]
""",
        encoding="utf-8",
    )

    # Generated posts intentionally opt out of sibling ordering. The starter
    # post template still has a real local index page to link back to.
    (POSTS_DIRECTORY / "index.page").write_text(
        """template = "section-index.tera"
title = "Performance fixture"
published = 2020-12-31T12:00:00
summary = "Deterministic synthetic pages for local performance profiling."
collections = []

[content]
main = "index.md"
""",
        encoding="utf-8",
    )
    (POSTS_DIRECTORY / "index.md").write_text(
        "# Performance fixture\n\nThis directory is generated locally and is not version-controlled.\n",
        encoding="utf-8",
    )


def post_body(generator: random.Random, words: int, number: int) -> str:
    tokens = [generator.choice(WORDS) for _ in range(words)]
    paragraphs = [" ".join(tokens[index : index + 50]) for index in range(0, words, 50)]
    return "\n\n".join((f"# Synthetic post {number}", *paragraphs, ""))


def write_post(generator: random.Random, words: int, number: int, published: date) -> None:
    stem = published.isoformat()
    tags = sorted(generator.sample(TAGS, k=generator.randint(1, 3)))
    updated = ""
    if generator.randrange(3) == 0:
        updated = f"updated = {published.isoformat()}T15:45:00\n"

    (POSTS_DIRECTORY / f"{stem}.page").write_text(
        "".join(
            (
                f'title = "Synthetic post {number:05}"\n',
                f"published = {published.isoformat()}T09:30:00\n",
                updated,
                f'summary = "Synthetic performance fixture post {number}."\n',
                "weight = false\n",
                "\n[taxonomies]\n",
                "tags = [",
                ", ".join(f'"{tag}"' for tag in tags),
                "]\n\n[extra]\n",
                f"fixture_number = {number}\n",
                f"fixture_variant = {generator.randrange(8)}\n",
                "\n[content]\n",
                f'main = "{stem}.md"\n',
            )
        ),
        encoding="utf-8",
    )
    (POSTS_DIRECTORY / f"{stem}.md").write_text(
        post_body(generator, words, number), encoding="utf-8"
    )


def generate(count: int, words: int, seed: int) -> None:
    reset_posts_directory()
    write_directory_descriptor()
    generator = random.Random(seed)
    for number in range(count):
        write_post(generator, words, number + 1, START_DATE - timedelta(days=number))
    print(f"Generated {count:,} posts in {POSTS_DIRECTORY.relative_to(SITE_DIRECTORY)}")


def clean() -> None:
    if POSTS_DIRECTORY.exists():
        shutil.rmtree(POSTS_DIRECTORY)
        print(f"Removed {POSTS_DIRECTORY.relative_to(SITE_DIRECTORY)}")
    else:
        print("Performance fixture is already absent")


def main() -> None:
    arguments = parse_arguments()
    if arguments.command == "generate":
        generate(arguments.count, arguments.words, arguments.seed)
    else:
        clean()


if __name__ == "__main__":
    main()
