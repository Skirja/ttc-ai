import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.assertEquals;

public final class JunitFailureTests {
    @Test
    void failingFixtureKeepsAssertionDiff() {
        assertEquals("expected", "actual");
    }
}
