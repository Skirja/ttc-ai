using TtcM7;
using Microsoft.VisualStudio.TestTools.UnitTesting;

[TestClass]
public sealed class CalculatorTests
{
    public static IEnumerable<object[]> GeneratedCases
    {
        get { return Enumerable.Range(0, 1001).Select(index => new object[] { index }); }
    }

    [TestMethod]
    [DynamicData(nameof(GeneratedCases))]
    public void AddsAnInteger(int value)
    {
        Assert.AreEqual(value + 2, Calculator.Add(value, 2));
    }

}
