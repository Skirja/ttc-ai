<?php

declare(strict_types=1);

use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\TestCase;
use TtcM7\Calculator;

final class CalculatorTest extends TestCase
{
    /**
     * @return iterable<string, array{int}>
     */
    public static function generatedCases(): iterable
    {
        for ($index = 0; $index < 1001; $index++) {
            yield sprintf('generated_%04d', $index) => [$index];
        }
    }

    #[DataProvider('generatedCases')]
    public function testAdditionForGeneratedInput(int $value): void
    {
        self::assertSame($value + 2, (new Calculator())->add($value, 2));
    }
}
