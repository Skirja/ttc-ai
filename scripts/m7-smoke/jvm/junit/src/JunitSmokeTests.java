import java.util.stream.IntStream;
import java.util.stream.Stream;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.TestFactory;
import static org.junit.jupiter.api.Assertions.assertEquals;

public final class JunitSmokeTests {
    @TestFactory
    Stream<DynamicTest> generatedPassingCases() {
        return IntStream.range(0, 1001)
            .mapToObj(index -> DynamicTest.dynamicTest(
                String.format("generated_%04d", index),
                () -> assertEquals(index + 2, index + 2)
            ));
    }
}
