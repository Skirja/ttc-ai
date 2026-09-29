<?php

declare(strict_types=1);

use TtcM7\Calculator;

it('adds two integers', function (): void {
    expect((new Calculator())->add(20, 22))->toBe(42);
});
