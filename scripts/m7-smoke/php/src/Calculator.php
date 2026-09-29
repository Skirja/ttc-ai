<?php

declare(strict_types=1);

namespace TtcM7;

/** @psalm-suppress UnusedClass */
final class Calculator
{
    public function add(int $left, int $right): int
    {
        return $left + $right;
    }
}
