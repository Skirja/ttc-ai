<?php

declare(strict_types=1);

use PHPUnit\Framework\TestCase;

final class FailureTest extends TestCase
{
    public function testFailureKeepsItsAssertionDiff(): void
    {
        self::assertSame('expected', 'actual');
    }
}
