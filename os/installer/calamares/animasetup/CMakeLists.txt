calamares_add_plugin(animasetup
    TYPE viewmodule
    EXPORT_MACRO PLUGINDLLEXPORT_PRO
    SOURCES Setup.cpp Storage.cpp Review.cpp Network.cpp NetworkConnection.cpp Hero.cpp
    SHARED_LIB
)
