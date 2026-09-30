using Microsoft.VisualStudio.TestTools.UnitTesting;

[TestClass]
public sealed class FailureTests
{
    [TestMethod]
    public void KeepsFailureDiagnostic()
    {
        var expected = Environment.GetEnvironmentVariable("M7_EXPECTED") ?? "expected";
        var actual = Environment.GetEnvironmentVariable("M7_ACTUAL") ?? "actual";
        Assert.AreEqual(expected, actual);
    }
}
